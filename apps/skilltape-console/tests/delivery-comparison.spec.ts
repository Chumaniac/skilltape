import { expect, test } from '@playwright/test'
import { compareDeliveries } from '../src/delivery-comparison'
import type { DeliveryReview, FileSummary } from '../src/types'

const file = (path: string, sha256 = 'a'.repeat(64)): FileSummary => ({ path, bytes: 10, sha256 })
const review = (id: string, files: FileSummary[]): DeliveryReview => ({
  schema: 'skilltape.dev/delivery-review/v1', id, status: 'passed', reported_status: 'succeeded',
  run_id: 'a'.repeat(64), receipt_binding: 'verified', requirement_validation: 'not-run', provenance: 'not-authenticated',
  declared_files: files.length, checked_files: files.length, declared_bytes: files.length * 10, checked_bytes: files.length * 10,
  files, files_truncated: false, findings: [], findings_truncated: false,
})

test('physical comparisons distinguish all changes and reject incomplete acceptance', () => {
  const a = review('a', [file('same'), file('changed'), file('removed')])
  const b = review('b', [file('same'), file('changed', 'b'.repeat(64)), file('added')])
  expect(compareDeliveries(a, b)?.counts).toEqual({ added: 1, removed: 1, changed: 1, unchanged: 1 })
  for (const invalid of [
    { ...b, status: 'failed' as const }, { ...b, receipt_binding: 'failed' as const },
    { ...b, files_truncated: true }, { ...b, findings_truncated: true },
    { ...b, declared_files: 4 }, { ...b, checked_bytes: 40 },
    { ...b, findings: [{ code: 'failure', path: null }] },
    review('b', [file('duplicate'), file('duplicate')]), review('b', [file('../unsafe')]),
    review('b', [file('invalid', 'not-a-hash')]),
  ]) expect(compareDeliveries(a, invalid)).toBeNull()
})

test('comparison summaries are bounded without hiding full counts', () => {
  const a = review('a', Array.from({ length: 100 }, (_, i) => file('before-' + i)))
  const b = review('b', Array.from({ length: 100 }, (_, i) => file('after-' + i)))
  const result = compareDeliveries(a, b)
  expect(result?.counts).toEqual({ added: 100, removed: 100, changed: 0, unchanged: 0 })
  expect(result?.changes).toHaveLength(100)
  expect(result?.truncated).toBe(true)
  expect(compareDeliveries(a, review('large', [...b.files, file('extra')]))).toBeNull()
})

test('an operator compares real response metadata and changing selection clears the result', async ({ page }) => {
  await page.route('**/api/v1/**', route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith('/deliveries')) return route.fulfill({ json: { schema: 'skilltape.dev/console/v1', items: ['before', 'after', 'incomplete'].map(id => ({ id, metadata_valid: true, reported_status: 'succeeded', run_id: 'a'.repeat(64), declared_files: 3, declared_bytes: 30 })), total: 3, offset: 0, limit: 50, next_offset: null } })
    const id = path.split('/').pop()!
    const r = review(id, id === 'before' ? [file('same'), file('changed'), file('removed')] : [file('same'), file('changed', 'b'.repeat(64)), file('added')])
    if (id === 'incomplete') r.files_truncated = true
    return route.fulfill({ json: r })
  })
  await page.goto('/#deliveries')
  const panel = page.locator('.delivery-comparison')
  await expect(panel.getByRole('button', { name: "比较文件变化" })).toBeDisabled()
  await page.getByLabel("基准交付包").selectOption('before')
  await page.getByLabel("替换交付包").selectOption('after')
  await panel.getByRole('button', { name: "比较文件变化" }).click()
  await expect(panel).toContainText("1 新增 · 1 移除 · 1 变化 · 1 未变化")
  await expect(panel.getByRole('columnheader', { name: "基准字节数", exact: true })).toBeVisible()
  await expect(panel.getByRole('columnheader', { name: "替换版本字节数", exact: true })).toBeVisible()
  await expect(panel.getByRole('row').filter({ has: page.getByText('changed', { exact: true }) }).getByRole('cell', { name: '10', exact: true })).toHaveCount(2)
  await expect(panel.getByText("要求 passed", { exact: true })).toHaveCount(0)
  await page.getByLabel("替换交付包").selectOption('incomplete')
  await expect(panel.getByText("文件对比完成")).toHaveCount(0)
  await panel.getByRole('button', { name: "比较文件变化" }).click()
  await expect(panel).toContainText("暂时无法对比")
  await page.setViewportSize({ width: 320, height: 720 })
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
})

test('obsolete comparison replies do not replace a later selection', async ({ page }) => {
  await page.route('**/api/v1/**', async route => {
    const id = new URL(route.request().url()).pathname.split('/').pop()!
    if (id === 'deliveries') return route.fulfill({ json: { schema: 'skilltape.dev/console/v1', items: ['baseline', 'slow', 'current'].map(id => ({ id, metadata_valid: true, reported_status: 'succeeded', run_id: 'a'.repeat(64), declared_files: 1, declared_bytes: 10 })), total: 3, offset: 0, limit: 50, next_offset: null } })
    if (id === 'slow') await new Promise(resolve => setTimeout(resolve, 300))
    return route.fulfill({ json: review(id, [file(id === 'slow' ? 'obsolete' : 'same')]) }).catch(() => undefined)
  })
  await page.goto('/#deliveries')
  await page.getByLabel("基准交付包").selectOption('baseline')
  await page.getByLabel("替换交付包").selectOption('slow')
  await page.getByRole('button', { name: "比较文件变化" }).click()
  await page.getByLabel("替换交付包").selectOption('current')
  await page.getByRole('button', { name: "比较文件变化" }).click()
  await expect(page.locator('.delivery-comparison')).toContainText("0 新增 · 0 移除 · 0 变化 · 1 未变化")
  await page.waitForTimeout(350)
  await expect(page.locator('.delivery-comparison')).not.toContainText('obsolete')
})
