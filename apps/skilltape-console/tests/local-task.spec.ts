import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
test.use({ locale: "en-US" });
const key = "skilltape.local-reconciliation.v1";
test("local reconciliation saves, restores and exports a calculation without a Receipt", async ({
  page,
}) => {
  await page.goto("/#task");
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(page.getByText("1 项差异", { exact: true })).toBeVisible();
  await page
    .getByRole("button", { name: "保存到本标签页", exact: true })
    .click();
  await page.reload();
  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "打开", exact: true }).click();
  await expect(page.getByText("1 项差异", { exact: true })).toBeVisible();
  const downloaded = page.waitForEvent("download");
  await page
    .getByRole("button", { name: "导出结果", exact: true })
    .click();
  const file = await downloaded;
  const data = JSON.parse(await readFile((await file.path())!, "utf8"));
  expect(data.balanced).toBe(false);
  expect(data).not.toHaveProperty("receipt");
});
test("edited and invalid input cannot keep an earlier calculated result", async ({
  page,
}) => {
  await page.goto("/#task");
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(page.getByText("1 项差异", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "编辑 JSON", exact: true }).click();
  await page.getByLabel("应有数量与实收数量（JSON）").fill("{bad");
  await expect(
    page.getByRole("button", { name: "导出结果", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(page.getByRole("status")).not.toContainText("计算完成");
  await expect(
    page.getByRole("button", { name: "保存到本标签页", exact: true }),
  ).toHaveCount(0);
});
test("corrupt saved history is preserved while new results remain exportable", async ({
  page,
}) => {
  await page.addInitScript(
    (k) => sessionStorage.setItem(k, "corrupt-original"),
    key,
  );
  await page.goto("/#task");
  await expect(page.getByRole("status")).toContainText(
    "原始存储已保留",
  );
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "保存到本标签页", exact: true }),
  ).toBeDisabled();
  expect(await page.evaluate((k) => sessionStorage.getItem(k), key)).toBe(
    "corrupt-original",
  );
  await expect(
    page.getByRole("button", { name: "导出结果", exact: true }),
  ).toBeEnabled();
});
test("the task is usable on a 320px screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto("/#task");
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(page.getByText("1 项差异", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth + 2,
    ),
  ).toBe(true);
});

test("item editor updates real quantities without requiring JSON", async ({
  page,
}) => {
  await page.goto("/#task");
  await expect(
    page.getByLabel("应有数量与实收数量（JSON）"),
  ).toHaveCount(0);
  await page.getByLabel("实收数量 1", { exact: true }).fill("5");
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(page.getByText("数量一致", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "添加物品", exact: true }).click();
  await page.getByLabel("物品名称 3", { exact: true }).fill("paper");
  await page.getByLabel("应有数量 3", { exact: true }).fill("2");
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(page.getByText("1 项差异", { exact: true })).toBeVisible();
  await page.getByLabel("应有数量 3", { exact: true }).fill("");
  await page
    .getByRole("button", { name: "计算差异", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "导出结果", exact: true }),
  ).toHaveCount(0);
});

test("difference-only viewing keeps the full result available for export", async ({ page }) => {
  await page.goto("/#task");
  await page.getByRole("button", { name: "计算差异", exact: true }).click();
  await expect(page.locator(".task-table tbody tr")).toHaveCount(2);
  await page.getByRole("checkbox", { name: "仅看差异", exact: true }).check();
  await expect(page.locator(".task-table tbody tr")).toHaveCount(1);
  await expect(page.locator(".task-table tbody")).toContainText("笔记本");
  const pending = page.waitForEvent("download");
  await page.getByRole("button", { name: "导出结果", exact: true }).click();
  const exported = JSON.parse(await readFile((await (await pending).path())!, "utf8"));
  expect(exported.rows).toHaveLength(2);
  expect(exported.rows.some((row: { item: string; difference: number }) => row.item === "铅笔" && row.difference === 0)).toBe(true);
  await page.getByRole("checkbox", { name: "仅看差异", exact: true }).uncheck();
  await expect(page.locator(".task-table tbody tr")).toHaveCount(2);
});

test("the interface stays Chinese in an English browser and JSON keeps stable keys", async ({ page }) => {
  await page.goto('/#task');
  await expect(page.locator('html')).toHaveAttribute('lang', 'zh-CN');
  await expect(page.getByRole('heading', { name: '核对数量', exact: true })).toBeVisible();
  await expect(page.getByLabel('物品名称 1', { exact: true })).toHaveValue('笔记本');
  await page.getByRole('button', { name: '编辑 JSON', exact: true }).click();
  const input = page.getByLabel('应有数量与实收数量（JSON）');
  const request = JSON.parse(await input.inputValue());
  expect(Object.keys(request).sort()).toEqual(['expected', 'received']);
  await input.fill('{bad');
  await page.getByRole('button', { name: '计算差异', exact: true }).click();
  await expect(page.getByRole('status')).toHaveText('JSON 格式有误，请检查括号、逗号和引号。');
  await expect(page.getByRole('button', { name: '导出结果', exact: true })).toHaveCount(0);
});

test("replacing a draft with the example respects a cancelled confirmation", async ({ page }) => {
  await page.goto("/#task");
  const item = page.getByLabel("物品名称 1", { exact: true });
  await item.fill("保留这份输入");
  page.once("dialog", dialog => dialog.dismiss());
  await page.getByRole("button", {name: "填入示例"}).click();
  await expect(item).toHaveValue("保留这份输入");
});

test("a history that becomes unreadable after opening is never overwritten", async ({ page }) => {
  await page.goto("/#task");
  await page.getByRole("button", {name: "计算差异"}).click();
  for (const raw of ["damaged", ""]) {
    await page.evaluate(([key, raw]) => sessionStorage.setItem(key, raw), [key, raw]);
    await page.getByRole("button", {name:"保存到本标签页"}).click();
    expect(await page.evaluate(key => sessionStorage.getItem(key), key)).toBe(raw);
    await expect(page.getByRole("status")).toContainText("原始存储已保留");
  }
});

test("CSV carries the filtered rows and treats a formula-like item as text", async ({ page }) => {
  await page.goto("/#task");
  const name = '=SUM(1,2)"纸"';
  await page.getByRole("button",{name:"编辑 JSON",exact:true}).click();
  await page.getByLabel("应有数量与实收数量（JSON）").fill(JSON.stringify({
    expected:[{item:name,quantity:4},{item:"相符",quantity:1}], received:[{item:"相符",quantity:1}]
  }));
  await page.getByRole("button",{name:"计算差异",exact:true}).click();
  await page.getByLabel("仅看差异").check();
  const [download] = await Promise.all([
    page.waitForEvent("download"), page.getByRole("button",{name:"导出当前表格 CSV"}).click()
  ]);
  expect(download.suggestedFilename()).toBe("reconciliation-differences.csv");
  expect(await readFile((await download.path())!, "utf8")).toBe("\uFEFF物品,应有数量,实收数量,差异\r\n\"'=SUM(1,2)\"\"纸\"\"\",4,0,-4\r\n");
});

test('the first-use path reveals invalid input and returns to the retained draft on mobile', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/#task');
  await page.getByRole('button', { name: '编辑 JSON', exact: true }).click();
  const draft = page.getByLabel('应有数量与实收数量（JSON）');
  await draft.fill('{not-json');
  await page.locator('.task-start-guide > summary').click();
  await page.getByRole('button', { name: '核对当前输入', exact: true }).click();
  await expect(page.locator('#task-result-panel')).toBeVisible();
  await expect(page.getByRole('status')).toContainText('JSON 格式有误');
  await page.getByRole('button', { name: '返回编辑清单', exact: true }).click();
  await expect(draft).toHaveValue('{not-json');
});
