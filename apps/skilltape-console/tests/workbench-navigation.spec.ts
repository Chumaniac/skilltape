import { expect, test } from '@playwright/test'

test('an unknown view has a recovery route and never loads an unrelated timeline', async ({ page }) => {
  const reads: string[] = []
  await page.route('**/api/v1/**', route => {
    reads.push(route.request().url())
    return route.fulfill({ status: 503, json: { error: { code: 'offline', message: 'API unavailable' } } })
  })
  await page.goto('/#not-a-console-view')
  await expect(page.getByRole('heading', { name: 'Page not found', exact: true })).toBeVisible()
  await expect(page.getByRole('link', { name: 'Open capture timeline', exact: true })).toHaveAttribute('href', '#timeline')
  expect(reads).toEqual([])
})

test('keyboard users can skip navigation without changing the active hash route', async ({ page }) => {
  await page.goto('/#export')
  await page.keyboard.press('Tab')
  await expect(page.getByRole('link', { name: 'Skip to content', exact: true })).toBeFocused()
  await page.keyboard.press('Enter')
  await expect(page.locator('#main-content')).toBeFocused()
  expect(new URL(page.url()).hash).toBe('#export')
})

test('an export without a chosen package asks for a real saved package', async ({ page }) => {
  const writes: string[] = []
  page.on('request', request => { if (request.method() !== 'GET') writes.push(request.method()) })
  await page.goto('/#export')
  await expect(page.getByRole('heading', { name: 'Choose a package to export', exact: true })).toBeVisible()
  await expect(page.getByLabel('Saved Skill ID')).toBeVisible()
  await expect(page.getByRole('region', {name:'Export command'})).toHaveCount(0)
  expect(writes).toEqual([])
})
