import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
const key = "skilltape.local-reconciliation.v1";
test("local reconciliation saves, restores and exports a calculation without a Receipt", async ({
  page,
}) => {
  await page.goto("/#task");
  await page
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(page.getByText("1 differences", { exact: true })).toBeVisible();
  await page
    .getByRole("button", { name: "Save in this tab", exact: true })
    .click();
  await page.reload();
  page.once("dialog", (d) => d.accept());
  await page.getByRole("button", { name: "Open", exact: true }).click();
  await expect(page.getByText("1 differences", { exact: true })).toBeVisible();
  const downloaded = page.waitForEvent("download");
  await page
    .getByRole("button", { name: "Export result", exact: true })
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
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(page.getByText("1 differences", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Edit JSON", exact: true }).click();
  await page.getByLabel("Expected and received quantities (JSON)").fill("{bad");
  await expect(
    page.getByRole("button", { name: "Export result", exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(page.getByRole("status")).not.toContainText("complete");
  await expect(
    page.getByRole("button", { name: "Save in this tab", exact: true }),
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
    "Original storage was kept",
  );
  await page
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Save in this tab", exact: true }),
  ).toBeDisabled();
  expect(await page.evaluate((k) => sessionStorage.getItem(k), key)).toBe(
    "corrupt-original",
  );
  await expect(
    page.getByRole("button", { name: "Export result", exact: true }),
  ).toBeEnabled();
});
test("the task is usable on a 320px screen", async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 844 });
  await page.goto("/#task");
  await page
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(page.getByText("1 differences", { exact: true })).toBeVisible();
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
    page.getByLabel("Expected and received quantities (JSON)"),
  ).toHaveCount(0);
  await page.getByLabel("Received 1", { exact: true }).fill("5");
  await page
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(page.getByText("Balanced", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Add item", exact: true }).click();
  await page.getByLabel("Item name 3", { exact: true }).fill("paper");
  await page.getByLabel("Expected 3", { exact: true }).fill("2");
  await page
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(page.getByText("1 differences", { exact: true })).toBeVisible();
  await page.getByLabel("Expected 3", { exact: true }).fill("");
  await page
    .getByRole("button", { name: "Calculate differences", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Export result", exact: true }),
  ).toHaveCount(0);
});
