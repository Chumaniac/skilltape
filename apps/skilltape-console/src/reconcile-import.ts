import { reconcile, type Request } from "./reconcile";

export const importLimit = 256000;
export type TableRow = { item: string; expected: string; received: string };

const isQuantity = (value: string) => /^\d+$/.test(value.trim());
// A byte order mark is what this page's own CSV export starts with.
const withoutMark = (text: string) => (text.charCodeAt(0) === 0xfeff ? text.slice(1) : text);

/** Splits delimited text into cells. Quoted cells may hold delimiters, quotes and line breaks. */
function cells(text: string, delimiter: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [], cell = "", quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (quoted) {
      if (char === '"' && text[index + 1] === '"') { cell += '"'; index += 1; }
      else if (char === '"') quoted = false;
      else cell += char;
    } else if (char === '"' && cell === "") quoted = true;
    else if (char === delimiter) { row.push(cell); cell = ""; }
    else if (char === "\n" || char === "\r") {
      if (char === "\r" && text[index + 1] === "\n") index += 1;
      row.push(cell); rows.push(row); row = []; cell = "";
    } else cell += char;
  }
  if (cell !== "" || row.length) { row.push(cell); rows.push(row); }
  return rows.filter(line => line.some(value => value.trim() !== ""));
}

/**
 * Reads rows of item / expected / received from spreadsheet text. Tabs win over commas so a
 * cell copied from a spreadsheet may contain commas; a heading row is skipped. A missing
 * received column means nothing was received. Values stay as text for the form to edit.
 */
export function parseTable(text: string): TableRow[] {
  const body = withoutMark(text);
  const delimiter = body.includes("\t") ? "\t" : body.includes(",") ? "," : body.includes(";") ? ";" : "\t";
  const lines = cells(body, delimiter);
  const first = lines[0];
  if (!first) throw Error("没有可导入的行。");
  // A heading has no number where a quantity belongs. A lone row is always data, so a
  // mistyped quantity is reported as such instead of being skipped as a heading.
  const heading = lines.length > 1 && first.length > 1 && !isQuantity(first[1] ?? "");
  const rows = (heading ? lines.slice(1) : lines).map(line => {
    const raw = (line[0] ?? "").trim();
    // CSV export prefixes formula-like names with an apostrophe; undo exactly that guard.
    const item = /^'\s*[=+@-]/u.test(raw) ? raw.slice(1) : raw;
    return { item, expected: (line[1] ?? "0").trim() || "0", received: (line[2] ?? "0").trim() || "0" };
  });
  if (rows.length > 1000) throw Error("最多导入1,000行。");
  return rows;
}

const quantity = (value: string) => (isQuantity(value) ? Number(value) : NaN);
const toRequest = (rows: TableRow[]): Request => ({
  expected: rows.map(row => ({ item: row.item, quantity: quantity(row.expected) })),
  received: rows.map(row => ({ item: row.item, quantity: quantity(row.received) })),
});

/**
 * Accepts what this page exports (task input, result, saved record, CSV) or a plain table.
 * The returned request has passed the same validation as a calculation.
 */
export function parseImport(text: string): Request {
  if (new TextEncoder().encode(text).length > importLimit) throw Error("输入上限为256 KB。");
  const trimmed = withoutMark(text).trim();
  if (!trimmed) throw Error("文件是空的。");
  let request: unknown;
  if (trimmed.startsWith("{")) {
    let data: { input?: unknown; rows?: unknown };
    try { data = JSON.parse(trimmed); } catch { throw Error("JSON 格式有误，请检查括号、逗号和引号。"); }
    if (data.input && typeof data.input === "object") request = data.input;
    else if (Array.isArray(data.rows)) {
      const rows = data.rows as Array<{ item: string; expected: number; received: number } | null>;
      request = {
        expected: rows.map(row => ({ item: row?.item, quantity: row?.expected })),
        received: rows.map(row => ({ item: row?.item, quantity: row?.received })),
      };
    } else request = data;
  } else request = toRequest(parseTable(trimmed));
  reconcile(request);
  const { expected, received } = request as Request;
  return {
    expected: expected.map(({ item, quantity: count }) => ({ item, quantity: count })),
    received: received.map(({ item, quantity: count }) => ({ item, quantity: count })),
  };
}
