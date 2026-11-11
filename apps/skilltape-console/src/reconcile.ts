export interface Quantity {
  item: string;
  quantity: number;
}
export interface Request {
  expected: Quantity[];
  received: Quantity[];
}
export function reconcile(value: unknown) {
  if (!value || typeof value !== "object")
    throw Error("请使用包含 expected 和 received 列表的对象。");
  const request = value as Request;
  if (
    !Array.isArray(request.expected) ||
    !request.expected.length ||
    request.expected.length > 1000 ||
    !Array.isArray(request.received) ||
    request.received.length > 1000
  )
    throw Error("应有列表须为1–1,000行，实收列表最多1,000行。");
  const totals = new Map<
    string,
    { item: string; expected: number; received: number }
  >();
  for (const side of ["expected", "received"] as const)
    for (const row of request[side]) {
      if (
        !row ||
        typeof row.item !== "string" ||
        !row.item.trim() ||
        new TextEncoder().encode(row.item).length > 128 ||
        /[\u0000-\u001f\u007f]/.test(row.item) ||
        !Number.isSafeInteger(row.quantity) ||
        row.quantity < 0 ||
        row.quantity > 1_000_000_000
      )
        throw Error(
          "每行须包含物品名称（1–128字节）及0至1,000,000,000之间的整数数量。",
        );
      const previous = totals.get(row.item) ?? {
        item: row.item,
        expected: 0,
        received: 0,
      };
      previous[side] += row.quantity;
      totals.set(row.item, previous);
    }
  const rows = [...totals.values()]
    .sort((a, b) => (a.item < b.item ? -1 : a.item > b.item ? 1 : 0))
    .map((r) => ({ ...r, difference: r.received - r.expected }));
  return {
    scenario: "reconcile" as const,
    balanced: rows.every((r) => r.difference === 0),
    rows,
  };
}
export const example: Request = {
  expected: [
    { item: "笔记本", quantity: 5 },
    { item: "铅笔", quantity: 10 },
  ],
  received: [
    { item: "笔记本", quantity: 4 },
    { item: "铅笔", quantity: 10 },
  ],
};
