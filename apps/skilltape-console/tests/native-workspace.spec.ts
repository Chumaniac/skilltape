import { expect, test } from '@playwright/test'
const key = 'skilltape.local-reconciliation.v1'

test('native explicit saves survive another window while browser retention stays session-only', async ({ context }) => {
  await context.addInitScript(() => Object.defineProperty(window, '__NATIVE_WORKSPACE__', { value: true }))
  const first = await context.newPage()
  await first.goto('/#task')
  await first.getByRole('button', { name: 'Calculate differences', exact: true }).click()
  await first.getByRole('button', { name: 'Save in this app', exact: true }).click()
  await expect(first.getByRole('status')).toHaveText('Saved in this app.')
  expect(await first.evaluate(k => localStorage.getItem(k), key)).not.toBeNull()
  expect(await first.evaluate(k => sessionStorage.getItem(k), key)).toBeNull()
  await first.close()
  const second = await context.newPage()
  await second.goto('/#task')
  await expect(second.getByRole('button', { name: 'Open', exact: true })).toHaveCount(1)
  second.once('dialog', dialog => dialog.accept())
  await second.getByRole('button', { name: 'Remove', exact: true }).click()
  await expect(second.getByText('No saved tasks.', { exact: true })).toBeVisible()
  expect(JSON.parse(await second.evaluate(k => localStorage.getItem(k)!, key)).items).toHaveLength(0)
})

test('unsaved quantity edits can cancel sidebar navigation', async ({ page }) => {
  await page.route('**/api/v1/workspaces**', route => route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[],offset:0,limit:50,total:0,next_offset:null}}))
  await page.goto('/#task')
  await page.getByRole('spinbutton', { name: 'Expected 1', exact: true }).fill('7')
  page.once('dialog', dialog => dialog.dismiss())
  await page.getByRole('link', { name: 'Capture timeline', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Reconcile quantities', exact: true })).toBeVisible()
  await expect(page.getByRole('spinbutton', { name: 'Expected 1', exact: true })).toHaveValue('7')
  page.once('dialog', dialog => dialog.accept())
  await page.getByRole('link', { name: 'Capture timeline', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Capture timeline', exact: true })).toBeVisible()
})
