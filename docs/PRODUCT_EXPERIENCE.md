# Reviewing a reconciliation

The quantity editor compares expected and received amounts. Export its JSON to prepare a local starter task. Only the local runner can produce a verification Receipt.

## Local workflow

The Console adds local quantity reconciliation: fill an example, edit item names and expected/received quantities, calculate a deterministic difference table, explicitly save tab history, reopen a calculation and export its result or starter input. JSON remains available as an advanced input mode. Blank or invalid quantities are refused and editing clears an obsolete result.

The original saved-evidence views and CLI capture, compile, lint, replay, verification and delivery commands remain available. A browser calculation is separate from verified evidence and never issues a Receipt. No browser command-execution API was added.

## Capacity and lifecycle

Input is limited to 256KB, with up to 1,000 rows on each side. Names and integer quantities are validated. Saved tab history is explicit, limited to 20 records and 512KB, and remains separate from the file-backed Receipt library. Invalid stored history is preserved while saving is disabled; current results can still be exported.

## Validation

The [task checks](../apps/skilltape-console/tests/local-task.spec.ts) cover quantity input, draft replacement, history preservation and export. The [native workspace checks](../apps/skilltape-console/tests/native-workspace.spec.ts) exercise application storage. Task input export writes the CLI fields `expected`, `received`, `item` and `quantity`; the saved-record export retains the original input.

## Local startup

```sh
cd apps/skilltape-console
npm ci
npm run dev -- --host 127.0.0.1
```

Choose **Reconcile quantities**. Follow the root README for the existing local evidence service and CLI capture workflow.

## Live local Console follow-up

A real local starter run exposed a navigation issue: opening a saved Receipt left its result below the search and comparison panels. The loaded result now receives focus and scrolls into view immediately. The document picker and read-only evidence boundaries remain available. Regression coverage checks the visible result at desktop, 390px and 320px widths.

## 工作区与窄屏操作

手机数量核对分为编辑清单、复核差异和保存记录三个工作区。计算成功后进入结果，切换工作区保留当前输入。桌面仍并排显示输入与结果。
