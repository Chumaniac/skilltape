import { useEffect, useRef, useState } from "react";
import { example, reconcile, type Request } from "../reconcile";
type FormRow = { item: string; expected: string; received: string };
const formRowsFor = (request: Request): FormRow[] =>
  reconcile(request).rows.map((r) => ({
    item: r.item,
    expected: String(r.expected),
    received: String(r.received),
  }));
type Result = ReturnType<typeof reconcile>;
type Saved = { id: string; at: string; input: Request; result: Result };
const key = "skilltape.local-reconciliation.v1";
const nativeWorkspace = () => typeof window !== "undefined" && (window as Window & { __NATIVE_WORKSPACE__?: boolean }).__NATIVE_WORKSPACE__ === true;
// Native saves survive application restarts; browser retention remains session-only.
const savedStorage = () => nativeWorkspace() ? localStorage : sessionStorage;
function read() {
  try {
    const raw = savedStorage().getItem(key);
    if (!raw) return { items: [] as Saved[], error: "" };
    if (raw.length > 512000) throw Error();
    const data = JSON.parse(raw);
    if (
      data.version !== 1 ||
      !Array.isArray(data.items) ||
      data.items.length > 20
    )
      throw Error();
    for (const item of data.items) {
      if (
        typeof item.id !== "string" ||
        item.id.length > 64 ||
        !Number.isFinite(Date.parse(item.at)) ||
        JSON.stringify(reconcile(item.input)) !== JSON.stringify(item.result)
      )
        throw Error();
    }
    return { items: data.items as Saved[], error: "" };
  } catch {
    return {
      items: [] as Saved[],
      error:
        "Saved local tasks could not be read. Original storage was kept; export your current result before changing storage.",
    };
  }
}
function download(value: unknown, name: string) {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
export function LocalTaskPage() {
  const [initial] = useState(read),
    [history, setHistory] = useState(initial.items),
    [input, setInput] = useState(JSON.stringify(example, null, 2)),
    [formRows, setFormRows] = useState<FormRow[]>(() => formRowsFor(example)),
    [jsonMode, setJsonMode] = useState(false),
    [result, setResult] = useState<Result | null>(null),
    [message, setMessage] = useState(initial.error),
    [busy, setBusy] = useState(false),
    [unsaved, setUnsaved] = useState(false),
    generation = useRef(0);
  useEffect(() => {
    const protect = (event: BeforeUnloadEvent) => { if (unsaved) { event.preventDefault(); event.returnValue = ""; } };
    window.addEventListener("beforeunload", protect);
    return () => window.removeEventListener("beforeunload", protect);
  }, [unsaved]);
  useEffect(() => () => { generation.current += 1; }, []);
  function editRows(next: FormRow[]) {
    setUnsaved(true);
    setFormRows(next);
    const number = (v: string) => (v.trim() === "" ? null : Number(v));
    setInput(
      JSON.stringify(
        {
          expected: next.map((r) => ({
            item: r.item,
            quantity: number(r.expected),
          })),
          received: next.map((r) => ({
            item: r.item,
            quantity: number(r.received),
          })),
        },
        null,
        2,
      ),
    );
    setResult(null);
    setMessage("Input changed. Calculate again to refresh the result.");
  }
  function showForm() {
    try {
      setFormRows(formRowsFor(JSON.parse(input)));
      setJsonMode(false);
    } catch {
      setMessage("Correct the JSON input before opening the item editor.");
    }
  }
  function run() {
    setUnsaved(true);
    setResult(null);
    setBusy(true);
    setMessage("Checking quantities…");
    const id = ++generation.current;
    setTimeout(() => {
      if (id !== generation.current) return;
      try {
        if (input.length > 256000) throw Error("Input is limited to 256 KB.");
        setResult(reconcile(JSON.parse(input)));
        setMessage(
          "Local calculation complete. Review differences before exporting.",
        );
      } catch (e) {
        setMessage(e instanceof Error ? e.message : "Input could not be read.");
      } finally {
        setBusy(false);
      }
    }, 0);
  }
  function save() {
    try {
      if (!result) return;
      if (initial.error) throw Error(initial.error);
      const record = {
        id: crypto.randomUUID(),
        at: new Date().toISOString(),
        input: JSON.parse(input) as Request,
        result,
      };
      const next = [record, ...history].slice(0, 20),
        raw = JSON.stringify({ version: 1, items: next });
      if (raw.length > 512000)
        throw Error(
          "Local history is full. Export this result before removing old records.",
        );
      savedStorage().setItem(key, raw);
      setHistory(next);
      setUnsaved(false);
      setMessage(nativeWorkspace() ? "Saved in this app." : "Saved in this tab. Export before closing it.");
    } catch (e) {
      setMessage(
        e instanceof Error
          ? e.message
          : "Storage is unavailable; export the result.",
      );
    }
  }
  return (
    <section className="page-section">
      <header className="page-heading">
        <div>
          <h1>Reconcile quantities</h1>
          <p className="page-description">
            Compare expected and received items. Start with an example, edit
            quantities, then review the differences.
          </p>
        </div>
      </header>
      <p className="task-scope">
        Runs on this device. This calculation is separate from captured runs
        and verified Receipts.
      </p>
      <div className="local-task-grid">
        <section className="task-panel">
          <h2>1. Input</h2>
          <button
            disabled={busy}
            onClick={() => {
              setInput(JSON.stringify(example, null, 2));
              setFormRows(formRowsFor(example));
              setJsonMode(false);
              setResult(null);
              setMessage("Example filled. No calculation has run.");
            }}
          >
            Fill example
          </button>
          <div className="task-actions">
            <button
              type="button"
              disabled={busy || !jsonMode}
              onClick={showForm}
            >
              Edit items
            </button>
            <button
              type="button"
              disabled={busy || jsonMode}
              onClick={() => setJsonMode(true)}
            >
              Edit JSON
            </button>
          </div>
          {!jsonMode ? (
            <>
              <div className="quantity-editor">
                {formRows.map((row, index) => (
                  <fieldset key={index} disabled={busy}>
                    <legend>Item {index + 1}</legend>
                    <label>
                      Item name
                      <input
                        aria-label={"Item name " + (index + 1)}
                        value={row.item}
                        maxLength={128}
                        onChange={(e) =>
                          editRows(
                            formRows.map((r, i) =>
                              i === index ? { ...r, item: e.target.value } : r,
                            ),
                          )
                        }
                      />
                    </label>
                    <label>
                      Expected
                      <input
                        aria-label={"Expected " + (index + 1)}
                        type="number"
                        min="0"
                        max="1000000000"
                        step="1"
                        value={row.expected}
                        onChange={(e) =>
                          editRows(
                            formRows.map((r, i) =>
                              i === index
                                ? { ...r, expected: e.target.value }
                                : r,
                            ),
                          )
                        }
                      />
                    </label>
                    <label>
                      Received
                      <input
                        aria-label={"Received " + (index + 1)}
                        type="number"
                        min="0"
                        max="1000000000"
                        step="1"
                        value={row.received}
                        onChange={(e) =>
                          editRows(
                            formRows.map((r, i) =>
                              i === index
                                ? { ...r, received: e.target.value }
                                : r,
                            ),
                          )
                        }
                      />
                    </label>
                    <button
                      type="button"
                      aria-label={"Remove item " + (index + 1)}
                      disabled={formRows.length <= 1}
                      onClick={() =>
                        editRows(formRows.filter((_, i) => i !== index))
                      }
                    >
                      Remove
                    </button>
                  </fieldset>
                ))}
              </div>
              <button
                type="button"
                disabled={busy || formRows.length >= 1000}
                onClick={() =>
                  editRows([
                    ...formRows,
                    { item: "", expected: "0", received: "0" },
                  ])
                }
              >
                Add item
              </button>
              <p>
                Enter item names and whole quantities. Use JSON for existing
                starter inputs.
              </p>
            </>
          ) : null}
          {jsonMode ? (
            <>
              <label htmlFor="task-input">
                Expected and received quantities (JSON)
              </label>
              <textarea
                id="task-input"
                rows={15}
                maxLength={256000}
                value={input}
                disabled={busy}
                onChange={(e) => {
                  setUnsaved(true);
                  setInput(e.target.value);
                  setResult(null);
                  setMessage(
                    "Input changed. Calculate again to refresh the result.",
                  );
                }}
              />
            </>
          ) : null}
          <p>
            Duplicate item names are added together. Quantities must be whole,
            nonnegative numbers.
          </p>
          <button className="button button-primary" disabled={busy} onClick={run}>
            {busy ? "Calculating…" : "Calculate differences"}
          </button>
        </section>
        <section className="task-panel">
          <h2>2. Result</h2>
          <p role="status">
            {message || "No result yet. Fill an example or enter quantities."}
          </p>
          {result ? (
            <>
              <div className="task-totals">
                <strong>{result.rows.length} items</strong>
                <span>
                  {result.rows.filter((r) => r.difference !== 0).length}{" "}
                  differences
                </span>
                <span>{result.balanced ? "Balanced" : "Review required"}</span>
              </div>
              <div className="task-table">
                <table>
                  <thead>
                    <tr>
                      <th>Item</th>
                      <th>Expected</th>
                      <th>Received</th>
                      <th>Difference</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result.rows.map((r) => (
                      <tr key={r.item}>
                        <td>{r.item}</td>
                        <td>{r.expected}</td>
                        <td>{r.received}</td>
                        <td>
                          {r.difference > 0 ? "+" : ""}
                          {r.difference}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div className="task-actions">
                <button onClick={save} disabled={!!initial.error} data-desktop-save="calculation">
                  {nativeWorkspace() ? "Save in this app" : "Save in this tab"}
                </button>
                <button
                  onClick={() => download(result, "reconciliation-result.json")}
                >
                  Export result
                </button>
                <button
                  onClick={() => download(JSON.parse(input), "request.json")}
                >
                  Export starter input
                </button>
              </div>
              <details>
                <summary>Use with the local capture workflow</summary>
                <p>
                  Create a reconcile starter with the CLI, replace its
                  inputs/request.json with the exported starter input, then run
                  the existing capture and verification workflow. A browser
                  calculation does not issue a Receipt.
                </p>
                <code>
                  skilltape starter new reconcile --output NEW_TASK_DIRECTORY
                </code>
              </details>
            </>
          ) : null}
        </section>
      </div>
      <section className="task-panel">
        <h2>{nativeWorkspace() ? "3. Saved in this app" : "3. Saved in this tab"}</h2>
        <p>{nativeWorkspace()
          ? "Up to 20 records / 512 KB. Explicit saves stay in this app until removed; local calculations remain separate from workspace evidence."
          : "Up to 20 records / 512 KB. Explicit saves only; closing this tab clears its session storage."}</p>
        {history.length ? (
          history.map((item) => (
            <div className="saved-task" key={item.id}>
              <span>
                {new Date(item.at).toLocaleString()} · {item.result.rows.length}{" "}
                items · {item.result.balanced ? "balanced" : "differences"}
              </span>
              <button
                onClick={() => {
                  if (
                    window.confirm(
                      "Replace the current input with this saved task?",
                    )
                  ) {
                    setInput(JSON.stringify(item.input, null, 2));
                    setFormRows(formRowsFor(item.input));
                    setJsonMode(false);
                    setResult(item.result);
                    setUnsaved(false);
                    setMessage("Opened a saved local calculation.");
                  }
                }}
              >
                Open
              </button>
              <button
                onClick={() => download(item, "saved-reconciliation.json")}
              >
                Export
              </button>
              <button
                onClick={() => {
                  if (window.confirm("Remove this saved local task?")) {
                    try {
                      const next = history.filter((n) => n.id !== item.id);
                      savedStorage().setItem(
                        key,
                        JSON.stringify({ version: 1, items: next }),
                      );
                      setHistory(next);
                    } catch {
                      setMessage("Could not update local history.");
                    }
                  }
                }}
              >
                Remove
              </button>
            </div>
          ))
        ) : (
          <p>No saved tasks.</p>
        )}
      </section>
    </section>
  );
}
