import { test, expect } from '@playwright/test'
test('a different API source never confirms installed capability parity', async ({ page }) => {
  await page.route('**/api/v1/build-info', route => route.fulfill({json:{schema:'skilltape.build-info/v1',version:'0.0.0',sourceRevision:'a'.repeat(40),dirty:false,channel:'source',capabilities:[]}}))
  await page.goto('/')
  await page.getByText('Installation details', { exact:false }).click()
  await expect(page.getByRole('status').filter({hasText:'Source differs'})).toBeVisible()
})
test('an unavailable API leaves capability comparison unconfirmed', async ({ page }) => {
  await page.route('**/api/v1/build-info', route => route.fulfill({status:503,json:{error:'unavailable'}}))
  await page.goto('/')
  await page.getByText('Installation details', { exact:false }).click()
  await expect(page.getByRole('status').filter({hasText:'No capability match is confirmed'})).toBeVisible()
})
