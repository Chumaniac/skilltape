import { expect, test } from '@playwright/test'

const permissions = {
  filesystem: { read: ['inputs/**'], write: ['outputs/**'] },
  process: { executables: ['/usr/bin/printf'], max_processes: 1, default_timeout_ms: 1000 },
  network: { enabled: false, allow_hosts: [] },
  secrets: { read_environment: false },
}

const skill = {
  schema: 'skilltape.dev/console/v1',
  id: 'demo',
  package_path: 'skills/demo',
  manifest: { name: 'demo', version: '0.1.0', targets: ['generic-agent-skill'] },
  workflow: {
    schema: 'skilltape.dev/workflow/v1',
    steps: [
      { action: 'exec', id: 'format', program: '/usr/bin/printf', args: ['ready'] },
      { action: 'file', id: 'copy', operation: 'copy', from: 'inputs/a', to: 'outputs/a' },
    ],
  },
  permissions,
  lockfile: { schema: 'skilltape.dev/lock/v1', tools: [] },
  files: [
    { path: 'skilltape.yaml', bytes: 220, sha256: 'a'.repeat(64) },
    { path: 'workflow.yaml', bytes: 320, sha256: 'b'.repeat(64) },
    { path: 'permissions.json', bytes: 280, sha256: 'c'.repeat(64) },
    { path: 'skilltape.lock', bytes: 100, sha256: 'd'.repeat(64) },
    { path: 'SKILL.md', bytes: 600, sha256: 'e'.repeat(64) },
    { path: 'README.md', bytes: 180, sha256: 'f'.repeat(64) },
  ],
  lint: { files_checked: 6, errors: [], warnings: [] },
}

const receipt = {
  schema: 'skilltape.dev/receipt/v1',
  run_id: 'd'.repeat(64),
  skill_hash: 'a'.repeat(64),
  status: 'succeeded',
  steps: [
    {
      step_id: 'format',
      status: 'succeeded',
      exit_code: 0,
      stdout_sha256: 'b'.repeat(64),
      stdout_bytes: 5,
      stdout_truncated: false,
      stderr_sha256: 'c'.repeat(64),
      stderr_bytes: 0,
      stderr_truncated: false,
    },
  ],
  assertions: [{ kind: 'file_exists', target: 'outputs/a', passed: true, reason: 'file exists' }],
  policy_decisions: [
    { step_id: 'format', phase: 'before', allowed: true, code: 'POLICY001', reason: 'allowed', risk: 'low' },
  ],
}

async function mockApi(page: import('@playwright/test').Page, mode: 'normal' | 'error' = 'normal', storedReceipt = receipt) {
  await page.route('**/api/v1/**', async (route) => {
    const url = new URL(route.request().url())
    if (mode === 'error' && url.pathname.endsWith('/workspaces')) {
      await route.fulfill({
        status: 503,
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/api-error/v1',
          error: { code: 'offline', message: 'Start the local Console API, then retry.' },
        }),
      })
      return
    }
    if (url.pathname.endsWith('/workspaces')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          items: [{ id: 'default', name: 'fixture-workspace', tape_count: 1, skill_count: 1, run_count: 1, receipt_count: 1 }],
          offset: 0,
          limit: 1,
          total: 1,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/workspaces/default/tapes')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          items: [
            {
              id: 'tape-a',
              schema: 'skilltape.dev/tape/v1',
              started_at_ms: 1710000000000,
              finished_at_ms: 1710000001000,
              platform: 'test',
              workspace_root: 'workspace',
              event_count: 1,
            },
          ],
          offset: 0,
          limit: 50,
          total: 1,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/tapes/none/events')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          tape_id: 'none',
          events: [],
          offset: 0,
          limit: 100,
          total: 0,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/tapes/tape-a/events')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          tape_id: 'tape-a',
          events: [
            {
              sequence: 0,
              occurred_at_ms: 1710000000500,
              kind: 'terminal_command',
              source: 'shell',
              payload: { command: 'printf', note: 'redacted timeline payload' },
              redaction: 'redacted',
            },
          ],
          offset: 0,
          limit: 100,
          total: 1,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/skills/demo/diff')) {
      await route.fulfill({ contentType: 'application/json', body: JSON.stringify(skill) })
      return
    }
    if (url.pathname.includes('/receipts/')) {
      if (!url.pathname.endsWith('/receipts/run-a')) {
        await route.fulfill({ status: 404, contentType: 'application/json', body: JSON.stringify({ error: { message: 'Saved Receipt not found.' } }) })
        return
      }
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ schema: 'skilltape.dev/receipt/v1', id: 'run-a', document: storedReceipt }),
      })
      return
    }
    await route.continue()
  })
}

test('timeline displays redaction state and payload details', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#timeline')

  await expect(page.getByRole('heading', { name: 'See the work as it happened' })).toBeVisible()
  await expect(page.getByText('Redacted', { exact: true })).toBeVisible()
  await page.getByText('Inspect event payload').click()
  await expect(page.getByText('redacted timeline payload')).toBeVisible()
  await expect(page.getByText('fixture-workspace')).toBeVisible()
})

test('timeline pages through tapes and events without accumulating earlier pages', async ({ page }, testInfo) => {
  await mockApi(page)
  let failNextPage = true
  await page.route('**/api/v1/workspaces/default/tapes*', async (route) => {
    const offset = Number(new URL(route.request().url()).searchParams.get('offset') ?? 0)
    const tapes = Array.from({ length: 51 }, (_, index) => ({
      id: 'tape-' + index,
      schema: 'skilltape.dev/tape/v1',
      started_at_ms: 0,
      finished_at_ms: 0,
      platform: 'test',
      workspace_root: 'workspace',
      event_count: 101,
    }))
    await route.fulfill({ json: {
      schema: 'skilltape.dev/console/v1', items: tapes.slice(offset, offset + 50),
      offset, limit: 50, total: tapes.length, next_offset: offset === 0 ? 50 : null,
    } })
  })
  await page.route('**/api/v1/tapes/tape-*/events*', async (route) => {
    const url = new URL(route.request().url())
    const offset = Number(url.searchParams.get('offset') ?? 0)
    if (offset === 100 && failNextPage) {
      failNextPage = false
      await route.fulfill({ status: 503, json: { error: { message: 'Synthetic page unavailable.' } } })
      return
    }
    const events = Array.from({ length: 101 }, (_, sequence) => ({
      sequence, occurred_at_ms: 0, kind: 'terminal_command', source: 'shell',
      payload: { note: 'synthetic event ' + sequence }, redaction: 'redacted',
    }))
    await route.fulfill({ json: {
      schema: 'skilltape.dev/console/v1', tape_id: url.pathname.split('/')[4],
      events: events.slice(offset, offset + 100), offset, limit: 100, total: events.length,
      next_offset: offset === 0 ? 100 : null,
    } })
  })

  await page.goto('/#timeline')
  await expect(page.locator('.event-row')).toHaveCount(100)
  await expect(page.getByRole('button', { name: 'Previous events' })).toBeDisabled()
  await expect(page.getByText('Finished', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Next events' }).click()
  await expect(page.getByRole('alert')).toContainText('Synthetic page unavailable.')
  await page.getByRole('button', { name: 'Retry request' }).click()
  await expect(page.locator('.event-row')).toHaveCount(1)
  await expect(page.getByText('#0100', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Next events' })).toBeDisabled()
  await page.getByRole('button', { name: 'Previous events' }).click()
  await expect(page.locator('.event-row')).toHaveCount(100)
  await expect(page.getByText('#0000', { exact: true })).toBeVisible()

  await page.getByRole('button', { name: 'Next tapes' }).click()
  await expect(page.getByRole('combobox', { name: 'Select capture tape' })).toHaveValue('tape-50')
  await expect(page.locator('select option')).toHaveCount(1)
  await expect(page.locator('.event-row')).toHaveCount(100)
  await page.getByRole('button', { name: 'Next events' }).click()
  await expect(page.getByText('#0100', { exact: true })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('combobox', { name: 'Select capture tape' })).toHaveValue('tape-50')
  await expect(page.getByText('#0100', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Previous tapes' }).click()
  await expect(page.getByRole('combobox', { name: 'Select capture tape' })).toHaveValue('tape-0')
  await page.getByRole('combobox', { name: 'Select capture tape' }).selectOption('tape-1')
  await expect(page.getByText('#0000', { exact: true })).toBeVisible()
  await expect(page.locator('.event-row')).toHaveCount(100)
  await page.screenshot({ path: testInfo.outputPath('pagination-desktop.png') })
  await page.setViewportSize({ width: 390, height: 844 })
  await expect(page.getByRole('button', { name: 'Next tapes' })).toBeVisible()
  await page.screenshot({ path: testInfo.outputPath('pagination-mobile.png') })
})

test('navigation opens compile and permission review pages', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#compile?skill=demo')
  await expect(page.getByRole('heading', { name: 'Inspect what the compiler produced' })).toBeVisible()
  await expect(page.getByText('Lint clean')).toBeVisible()
  await expect(page.getByText('format', { exact: true })).toBeVisible()

  await page.getByRole('link', { name: 'Permissions' }).click()
  await expect(page.getByRole('heading', { name: 'Know what the Skill is allowed to touch' })).toBeVisible()
  await expect(page.getByText('Environment variables are not available.')).toBeVisible()
  await expect(page.getByText('Scoped access', { exact: true })).toBeVisible()
})

test('receipt view shows bounded evidence and policy decisions', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#receipt?run=run-a')

  await expect(page.getByRole('heading', { name: 'Evidence you can inspect, not just trust' })).toBeVisible()
  await expect(page.locator('.page-heading .status-badge').getByText('Succeeded', { exact: true })).toBeVisible()
  await expect(page.getByText('Output redacted')).toBeVisible()
  await expect(page.locator('.decision-list').getByText('POLICY001', { exact: false })).toBeVisible()
  await expect(page.locator('.result-list').getByText('File Exists', { exact: true })).toBeVisible()
})

test('receipt review filters failed output checks and keeps the complete outcome visible', async ({ page }, testInfo) => {
  await mockApi(page, 'normal', {
    ...receipt, status: 'assertion_failed', assertions: [
      { kind: 'file_exists', target: 'outputs/report.csv', passed: true, reason: 'file exists' },
      { kind: 'file_hash', target: 'outputs/report.csv', passed: false, reason: 'output hash does not match expected digest' },
      { kind: 'command_exit', target: 'export', passed: true, reason: 'exit code matches' },
    ],
  })
  await page.goto('/#receipt?run=run-a')
  await expect(page.getByRole('status').filter({ hasText: '2 passed · 1 failed' })).toBeVisible()
  await page.getByRole('button', { name: 'Needs attention (1)', exact: true }).click()
  await expect(page.locator('.result-list li')).toHaveCount(1)
  await expect(page.locator('.result-list')).toContainText('File Hash')
  await expect(page.locator('.result-list')).toContainText('Failed')
  await expect(page.locator('.page-heading')).toContainText('Assertion Failed')
  await page.screenshot({ path: testInfo.outputPath('receipt-output-review-desktop.png'), fullPage: true })
  await page.getByRole('button', { name: 'All checks (3)', exact: true }).click()
  await expect(page.locator('.result-list li')).toHaveCount(3)
  await expect(page.locator('.result-list').getByText('Passed', { exact: true })).toHaveCount(2)
})

test('saved Receipt picker recovers from missing IDs and rejects path input without a request', async ({ page }) => {
  const requests: string[] = []
  page.on('request', request => { if (request.url().includes('/api/v1/receipts/')) requests.push(request.url()) })
  await mockApi(page)
  await page.goto('/#receipt?run=missing')
  await expect(page.getByRole('alert')).toContainText('Saved Receipt not found')
  const input = page.getByLabel('Saved Receipt ID', { exact: true })
  await input.fill('../private')
  await page.getByRole('button', { name: 'Open Receipt', exact: true }).click()
  await expect(page.getByText('Use a saved filename, not a path.', { exact: true })).toBeVisible()
  expect(requests).toHaveLength(1)
  await input.fill('run-a.json')
  await input.press('Enter')
  await expect(page.locator('.result-list')).toContainText('File Exists')
  await expect(page).toHaveURL(/#receipt\?run=run-a$/)
  await expect(page.getByRole('button', { name: 'All checks (1)', exact: true })).toHaveAttribute('aria-pressed', 'true')
})

test('malformed or oversized stored Receipts show a recoverable error', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  const cases = [
    { ...receipt, assertions: [{ type: 'file_hash', target: 'output', passed: true, reason: 'legacy fixture field' }] },
    { ...receipt, assertions: Array.from({ length: 1001 }, () => receipt.assertions[0]) },
    { ...receipt, steps: [{ ...receipt.steps[0], stdout_bytes: -1 }] },
    { ...receipt, assertions: [{ ...receipt.assertions[0], passed: false }] },
    { ...receipt, raw_output: 'private-sentinel-not-for-display' },
  ]
  for (const document of cases) {
    await page.route('**/api/v1/receipts/**', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify({ schema: receipt.schema, id: 'run-a', document }) }))
    await page.goto('/#receipt?run=run-a')
    await expect(page.getByRole('alert')).toContainText('invalid or unsupported')
    await expect(page.getByRole('button', { name: 'Open Receipt', exact: true })).toBeVisible()
    await expect(page.locator('.result-list')).toHaveCount(0)
    await expect(page.locator('body')).not.toContainText('private-sentinel-not-for-display')
    await page.unroute('**/api/v1/receipts/**')
  }
  expect(errors).toEqual([])
})

test('opening a different saved Receipt clears the previous failure filter', async ({ page }) => {
  await mockApi(page, 'normal', { ...receipt, status: 'assertion_failed', assertions: [{ ...receipt.assertions[0], passed: false }] })
  await page.route('**/api/v1/receipts/run-b', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify({ schema: receipt.schema, id: 'run-b', document: { ...receipt, run_id: 'e'.repeat(64) } }) }))
  await page.goto('/#receipt?run=run-a')
  await page.getByRole('button', { name: 'Needs attention (1)', exact: true }).click()
  await page.getByLabel('Saved Receipt ID', { exact: true }).fill('run-b')
  await page.getByRole('button', { name: 'Open Receipt', exact: true }).click()
  await expect(page.getByRole('button', { name: 'All checks (1)', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.locator('.result-list')).toContainText('Passed')
  await expect(page.locator('.page-heading')).toContainText('Succeeded')
})

test('receipt details render literal untrusted text and remain readable on mobile', async ({ page }, testInfo) => {
  const target = 'outputs/' + 'long-file-name-'.repeat(24) + '<img src=x onerror=alert(1)>.csv'
  const reason = '<script>window.receiptInjected=true</script> digest mismatch'
  await mockApi(page, 'normal', { ...receipt, status: 'assertion_failed', assertions: [{ kind: 'file_hash', target, passed: false, reason }] })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/#receipt?run=run-a')
  await expect(page.locator('.result-list')).toContainText(target)
  await expect(page.locator('.result-list')).toContainText(reason)
  await expect(page.locator('.result-list img, .result-list script')).toHaveCount(0)
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true)
  await page.screenshot({ path: testInfo.outputPath('receipt-output-review-mobile.png'), fullPage: true })
})

test('empty and unavailable states explain the next step', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#timeline?tape=none')
  await expect(page.getByRole('heading', { name: 'This Tape is empty' })).toBeVisible()

  await mockApi(page, 'error')
  await page.goto('/#timeline?retry=1')
  await expect(page.getByRole('alert')).toContainText('Start the local Console API')
  await expect(page.getByRole('button', { name: 'Retry request' })).toBeVisible()
})
