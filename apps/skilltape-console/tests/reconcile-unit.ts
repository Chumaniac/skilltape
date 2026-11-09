import assert from "node:assert/strict";
import { reconcile, example } from "../src/reconcile.ts";
const result = reconcile(example);
assert.equal(result.balanced, false);
assert.equal(result.rows[0]?.difference, -1);
assert.equal(
  reconcile({
    expected: [
      { item: "a", quantity: 1 },
      { item: "a", quantity: 2 },
    ],
    received: [{ item: "a", quantity: 3 }],
  }).balanced,
  true,
);
assert.throws(() =>
  reconcile({ expected: [{ item: "a", quantity: -1 }], received: [] }),
);
assert.throws(() => reconcile({ expected: [], received: [] }));
assert.throws(() =>
  reconcile({ expected: [{ item: "a", quantity: Infinity }], received: [] }),
);
assert.throws(() =>
  reconcile({
    expected: [{ item: "a".repeat(129), quantity: 1 }],
    received: [],
  }),
);
assert.equal(
  reconcile({
    expected: [{ item: "a", quantity: 0 }],
    received: [{ item: "<script>", quantity: 3 }],
  }).rows.length,
  2,
);
console.log(
  "PASS: reconciliation, duplicate totals, literal item names and invalid inputs",
);
