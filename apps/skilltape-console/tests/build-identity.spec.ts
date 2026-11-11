import { test, expect } from '@playwright/test'
test('a different API source never confirms installed capability parity', async ({ page }) => {
  await page.route('**/api/v1/build-info', route => route.fulfill({json:{schema:'skilltape.build-info/v1',version:'0.0.0',sourceRevision:'a'.repeat(40),dirty:false,channel:'source',capabilities:[]}}))
  await page.goto('/')
  await page.getByText('安装详情', { exact:false }).click()
  await expect(page.getByRole('status').filter({hasText:'源码不同或包含本地改动'})).toBeVisible()
})
test('an unavailable API leaves capability comparison unconfirmed', async ({ page }) => {
  await page.route('**/api/v1/build-info', route => route.fulfill({status:503,json:{error:'unavailable'}}))
  await page.goto('/')
  await page.getByText('安装详情', { exact:false }).click()
  await expect(page.getByRole('status').filter({hasText:'尚未确认能力一致'})).toBeVisible()
})
