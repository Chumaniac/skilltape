# Reviewing a reconciliation

The quantity editor compares expected and received amounts. Export its JSON to prepare a local starter task. Only the local runner can produce a verification Receipt.

## Local workflow

The Console adds local quantity reconciliation: fill an example, edit item names and expected/received quantities, calculate a deterministic difference table, explicitly save tab history, reopen a calculation and export its result or starter input. JSON remains available as an advanced input mode. Blank or invalid quantities are refused and editing clears an obsolete result.

The original saved-evidence views and CLI capture, compile, lint, replay, verification and delivery commands remain available. A browser calculation is separate from verified evidence and never issues a Receipt. No browser command-execution API was added.

## Capacity and lifecycle

Input is limited to 256KB, with up to 1,000 rows on each side. Names and integer quantities are validated. Saved tab history is explicit, limited to 20 records and 512KB, and remains separate from the file-backed Receipt library. Invalid stored history is preserved while saving is disabled; current results can still be exported.

## Validation

Frontend build and type checks, quantity boundary checks, 80 existing Console browser checks and five new local-task checks passed. New coverage includes guided input, save/reopen/download, stale-result invalidation, preserved corrupt history and 320px layouts. Browser flows were also checked at desktop and 390px widths. These checks do not establish provider or production acceptance.

## Local startup

```sh
cd apps/skilltape-console
npm ci
npm run dev -- --host 127.0.0.1
```

Choose **Reconcile quantities**. Follow the root README for the existing local evidence service and CLI capture workflow.

## Live local Console follow-up

A real local starter run exposed a navigation issue: opening a saved Receipt left its result below the search and comparison panels. The loaded result now receives focus and scrolls into view immediately. The document picker and read-only evidence boundaries remain available. Regression coverage checks the visible result at desktop, 390px and 320px widths.
