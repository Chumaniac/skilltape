# Product experience

The workbench is a local efficiency tool. Show the current task, its inputs, results and next action. Examples fill or preview inputs; they do not submit work or grant permissions. Keep synthetic data, local calculations, saved evidence and live results distinct.

## Visual rules

Use a neutral background (#f7f7f5), white surfaces, dark text (#242424), secondary text (#62655f), thin borders (#dedfd9) and a restrained action color (#31594d). Preserve warning and error colors. Use system fonts and existing local font assets; do not add remote font requests.

Body text is 14-16px with generous line height. Use clear headings, spacing in 8px steps, small corners and minimal shadows. Use approximately 200ms color and border transitions and respect reduced-motion preferences. Keep visible keyboard focus, labelled controls and status messages. Main touch actions are at least 44px; narrow screens use one column and bounded table scrolling.

## Data and interaction rules

Provide examples, explicit run/save/export actions, useful empty states and actionable errors. Retain inputs after failures and invalidate old results after input changes. Bound history, input sizes and retained resources. Preserve invalid stored data rather than silently resetting it. Never include credentials in reports or diagnostics.

The supplied reference page was not accessible through the available reader. These changes follow the user's written description of a restrained, practical product style.

## Implemented workflow

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
