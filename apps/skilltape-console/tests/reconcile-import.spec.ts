import { test, expect } from "@playwright/test";
import { parseImport, parseTable } from "../src/reconcile-import";

const mark = String.fromCharCode(0xfeff);

test("spreadsheet text becomes editable rows with or without a heading", () => {
  expect(parseTable("物品\t应有数量\t实收数量\n笔记本\t5\t4\n铅笔, HB\t10\t10\n")).toEqual([
    { item: "笔记本", expected: "5", received: "4" },
    { item: "铅笔, HB", expected: "10", received: "10" },
  ]);
  // No heading, no received column: nothing was received yet.
  expect(parseTable("纸\t3\r\n\r\n笔\t2")).toEqual([
    { item: "纸", expected: "3", received: "0" },
    { item: "笔", expected: "2", received: "0" },
  ]);
  expect(parseTable("纸;3;1")).toEqual([{ item: "纸", expected: "3", received: "1" }]);
  expect(() => parseTable(" \n ")).toThrow("没有可导入的行");
  // A lone row is data even when its quantity is not a number; validation reports it later.
  expect(parseTable("纸,three")).toEqual([{ item: "纸", expected: "three", received: "0" }]);
  expect(() => parseTable(Array.from({ length: 1001 }, (_, index) => `item${index},1,1`).join("\n"))).toThrow("最多导入1,000行");
});

test("the page's own CSV export round-trips, including quoted and formula-like names", () => {
  const exported = mark + "物品,应有数量,实收数量,差异\r\n\"'=SUM(1,2)\"\"纸\"\"\",4,0,-4\r\n\"多行\n名称, 含逗号\",2,2,0\r\n";
  expect(parseTable(exported)).toEqual([
    { item: '=SUM(1,2)"纸"', expected: "4", received: "0" },
    { item: "多行\n名称, 含逗号", expected: "2", received: "2" },
  ]);
  // An ordinary leading apostrophe is part of the name and stays.
  expect(parseTable("'tis,1,1")[0]?.item).toBe("'tis");
});

test("imports accept task input, results, saved records and tables, and nothing invalid", () => {
  const request = { expected: [{ item: "a", quantity: 2 }], received: [{ item: "a", quantity: 1 }] };
  expect(parseImport(JSON.stringify(request))).toEqual(request);
  // Extra members of an exported file are not carried into the task input.
  expect(parseImport(JSON.stringify({ id: "x", at: "2026-11-21T00:00:00Z", input: request, result: {} }))).toEqual(request);
  expect(parseImport(JSON.stringify({ scenario: "reconcile", balanced: false, rows: [{ item: "a", expected: 2, received: 1, difference: -1 }] }))).toEqual(request);
  expect(parseImport(mark + "物品,应有数量,实收数量\na,2,1\n")).toEqual(request);

  expect(() => parseImport("a,1,1\nb,two,1")).toThrow("每行须包含物品名称");
  expect(() => parseImport("a,-2,1")).toThrow("每行须包含物品名称");
  expect(() => parseImport("a,1.5,1")).toThrow("每行须包含物品名称");
  expect(() => parseImport("{bad")).toThrow("JSON 格式有误");
  expect(() => parseImport(JSON.stringify({ rows: [null] }))).toThrow("每行须包含物品名称");
  expect(() => parseImport(JSON.stringify({ expected: [], received: [] }))).toThrow("应有列表须为1–1,000行");
  expect(() => parseImport("   ")).toThrow("文件是空的");
  expect(() => parseImport("a,1,1\n".repeat(50000))).toThrow("输入上限为256 KB");
});
