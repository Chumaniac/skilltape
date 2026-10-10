import { expect, test } from '@playwright/test'

const summary = (id: string) => ({ id, metadata_valid: true, reported_status: 'succeeded',
  run_id: 'a'.repeat(64), declared_files: 3, declared_bytes: 300 })
const review = (id: string, failed = false) => ({ schema: 'skilltape.dev/delivery-review/v1', id,
  status: failed ? 'failed' : 'passed', reported_status: 'succeeded', run_id: 'a'.repeat(64),
  receipt_binding: 'verified', requirement_validation: 'not-run', provenance: 'not-authenticated',
  declared_files: 3, declared_bytes: 300, checked_files: failed ? 2 : 3, checked_bytes: failed ? 200 : 300,
  files: [{ path: 'expected.csv', bytes: 100, sha256: 'b'.repeat(64) }], files_truncated: false,
  findings: failed ? [{ code: 'delivery.artifact-digest-or-snapshot', path: 'allocations.csv' }] : [], findings_truncated: false })

test('saved deliveries can be selected and rechecked without pretending requirements passed', async ({ page }) => {
  let checks = 0
  await page.route('**/api/v1/**', async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith('/deliveries')) return route.fulfill({ json: { schema: 'skilltape.dev/console/v1',
      items: [summary('saved-order')], total: 1, offset: 0, limit: 50, next_offset: null } })
    if (path.endsWith('/deliveries/saved-order')) return route.fulfill({ json: review('saved-order', ++checks > 1) })
    return route.fulfill({ json: { items: [], total: 0, offset: 0, limit: 50, next_offset: null } })
  })
  await page.goto('/#deliveries')
  await expect(page.getByRole('heading', { name: 'Review saved deliveries' })).toBeVisible()
  await page.getByRole('link', { name: 'Inspect saved-order' }).click()
  await expect(page.getByText('Physical files match', { exact: true })).toBeVisible()
  await expect(page.getByText('Requirements not checked', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Recheck selected delivery' }).click()
  await expect(page.getByText('Physical files need attention', { exact: true })).toBeVisible()
  await expect(page.getByText('allocations.csv', { exact: true })).toBeVisible()
  expect(checks).toBe(2)
})

test('pages stay bounded and no delivery is silently selected', async ({ page }) => {
  await page.route('**/api/v1/**', async route => {
    const url = new URL(route.request().url())
    const offset = Number(url.searchParams.get('offset') ?? 0)
    return route.fulfill({ json: { schema: 'skilltape.dev/console/v1',
      items: Array.from({ length: 51 }, (_, index) => summary('saved-' + index)).slice(offset, offset + 50),
      total: 51, offset, limit: 50, next_offset: offset === 0 ? 50 : null } })
  })
  await page.goto('/#deliveries')
  await expect(page.locator('[data-delivery-row]')).toHaveCount(50)
  await expect(page.getByRole('heading', { name: 'Choose a saved delivery' })).toBeVisible()
  await page.getByRole('button', { name: 'Next deliveries' }).click()
  await expect(page.locator('[data-delivery-row]')).toHaveCount(1)
  await expect(page.getByRole('link', { name: 'Inspect saved-50' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Next deliveries' })).toBeDisabled()
  await page.getByRole('button', { name: 'Previous deliveries' }).click()
  await expect(page.locator('[data-delivery-row]')).toHaveCount(50)
})

test('empty and unavailable roots show actions without demo fallback', async ({ page }) => {
  let unavailable = false
  await page.route('**/api/v1/**', async route => {
    if (unavailable) return route.fulfill({ status: 503, json: { error: { message: 'Synthetic inspection unavailable.' } } })
    return route.fulfill({ json: { items: [], total: 0, offset: 0, limit: 50, next_offset: null } })
  })
  await page.goto('/#deliveries')
  await expect(page.getByRole('heading', { name: 'No saved deliveries found' })).toBeVisible()
  await expect(page.getByText('run-a', { exact: true })).toHaveCount(0)
  unavailable = true
  await page.getByRole('button', { name: 'Refresh delivery list' }).click()
  await expect(page.getByRole('alert')).toContainText('Synthetic inspection unavailable.')
})

test('late inspection replies cannot replace the newly selected delivery', async ({ page }) => {
  await page.route('**/api/v1/**', async route => {
    const url = new URL(route.request().url())
    if (url.pathname.endsWith('/deliveries')) return route.fulfill({ json: { items: [summary('slow'), summary('current')],
      schema: 'skilltape.dev/console/v1', total: 2, offset: 0, limit: 50, next_offset: null } })
    if (url.pathname.endsWith('/slow')) {
      await new Promise(resolve => setTimeout(resolve, 400))
      return route.fulfill({ json: review('slow', true) }).catch(() => undefined)
    }
    return route.fulfill({ json: review('current') })
  })
  await page.goto('/#deliveries')
  await page.getByRole('link', { name: 'Inspect slow' }).click()
  await page.getByRole('link', { name: 'Inspect current' }).click()
  await expect(page.locator('.delivery-selected h2')).toHaveText('current')
  await expect(page.getByText('Physical files match', { exact: true })).toBeVisible()
  await page.waitForTimeout(500)
  await expect(page.locator('.delivery-selected h2')).toHaveText('current')
  await expect(page.getByText('Physical files need attention', { exact: true })).toHaveCount(0)
})

test('malformed successful responses do not become empty or accepted evidence', async ({ page }) => {
  await page.route('**/api/v1/**', route => route.fulfill({ json: { unexpected: true } }))
  await page.goto('/#deliveries')
  await expect(page.getByRole('alert')).toContainText('invalid or unsupported')
  await expect(page.getByRole('heading', { name: 'No saved deliveries found' })).toHaveCount(0)
  await expect(page.getByText('Physical files match', { exact: true })).toHaveCount(0)
})
