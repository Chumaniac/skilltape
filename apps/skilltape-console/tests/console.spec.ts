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
  run_id: 'run-a',
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
  assertions: [{ type: 'file_exists', target: 'outputs/a', passed: true, reason: 'file exists' }],
  policy_decisions: [
    { step_id: 'format', phase: 'before', allowed: true, code: 'POLICY001', reason: 'allowed', risk: 'low' },
  ],
}

async function mockApi(page: import('@playwright/test').Page, mode: 'normal' | 'error' = 'normal') {
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
    if (url.pathname.endsWith('/receipts/run-a')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ schema: 'skilltape.dev/receipt/v1', id: 'run-a', document: receipt }),
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

test('empty and unavailable states explain the next step', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#timeline?tape=none')
  await expect(page.getByRole('heading', { name: 'This Tape is empty' })).toBeVisible()

  await mockApi(page, 'error')
  await page.goto('/#timeline?retry=1')
  await expect(page.getByRole('alert')).toContainText('Start the local Console API')
  await expect(page.getByRole('button', { name: 'Retry request' })).toBeVisible()
})
