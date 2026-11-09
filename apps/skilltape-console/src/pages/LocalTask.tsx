import { useRef, useState } from "react";
import { example, reconcile, type Request } from "../reconcile";
type Result = ReturnType<typeof reconcile>;
type Saved = { id: string; at: string; input: Request; result: Result };
const key = "skilltape.local-reconciliation.v1";
function read() {
  try {
    const raw = sessionStorage.getItem(key);
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
    [result, setResult] = useState<Result | null>(null),
    [message, setMessage] = useState(initial.error),
    [busy, setBusy] = useState(false),
    generation = useRef(0);
  function run() {
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
      sessionStorage.setItem(key, raw);
      setHistory(next);
      setMessage("Saved in this tab. Export before closing it.");
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
        Runs in this browser. This calculation is separate from captured runs
        and verified Receipts.
      </p>
      <div className="local-task-grid">
        <section className="task-panel">
          <h2>1. Input</h2>
          <button
            disabled={busy}
            onClick={() => {
              setInput(JSON.stringify(example, null, 2));
              setResult(null);
              setMessage("Example filled. No calculation has run.");
            }}
          >
            Fill example
          </button>
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
              setInput(e.target.value);
              setResult(null);
              setMessage(
                "Input changed. Calculate again to refresh the result.",
              );
            }}
          />
          <p>
            Duplicate item names are added together. Quantities must be whole,
            nonnegative numbers.
          </p>
          <button className="button" disabled={busy} onClick={run}>
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
                <button onClick={save} disabled={!!initial.error}>
                  Save in this tab
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
        <h2>3. Saved in this tab</h2>
        <p>
          Up to 20 records / 512 KB. Explicit saves only; closing this tab
          clears its session storage.
        </p>
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
                    setResult(item.result);
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
                      sessionStorage.setItem(
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
