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
        "无法读取本地历史。原始存储已保留，修改存储前请先导出当前结果。",
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
    [onlyDifferences, setOnlyDifferences] = useState(false),
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
    setMessage("输入已改变，请重新计算以更新结果。");
  }
  function showForm() {
    try {
      setFormRows(formRowsFor(JSON.parse(input)));
      setJsonMode(false);
    } catch {
      setMessage("请先修正 JSON 输入，再打开物品编辑器。");
    }
  }
  function run() {
    setUnsaved(true);
    setResult(null);
    setBusy(true);
    setMessage("正在核对数量…");
    const id = ++generation.current;
    setTimeout(() => {
      if (id !== generation.current) return;
      try {
        if (input.length > 256000) throw Error("输入上限为256 KB。");
        setResult(reconcile(JSON.parse(input)));
        setMessage(
          "本机计算完成，请在导出前复核差异。",
        );
      } catch (e) {
        setMessage(e instanceof SyntaxError ? "JSON 格式有误，请检查括号、逗号和引号。" : e instanceof Error ? e.message : "无法读取输入。");
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
          "本地历史已满，请先导出当前结果，再移除旧记录。",
        );
      savedStorage().setItem(key, raw);
      setHistory(next);
      setUnsaved(false);
      setMessage(nativeWorkspace() ? "已保存到本应用。" : "已保存到本标签页，关闭前请导出。");
    } catch (e) {
      setMessage(
        e instanceof Error
          ? e.message
          : "存储暂不可用，请导出结果。",
      );
    }
  }
  const differenceCount = result?.rows.filter(row => row.difference !== 0).length ?? 0;
  const visibleRows = result?.rows.filter(row => !onlyDifferences || row.difference !== 0) ?? [];
  return (
    <section className="page-section local-task-workspace">
      <header className="page-heading task-hero">
        <div>
          <p className="eyebrow">数量核对工作台</p>
          <h1>核对数量</h1>
          <p className="page-description">
            核对物品的应有数量与实收数量。可先填入示例，修改数量后查看差异。
          </p>
        </div>
        <span className="task-mode-badge">本机计算</span>
      </header>
      <div className="task-workspace-stats" aria-label="当前计算概览">
        <div className="task-stat task-stat-accent"><span>输入行数</span><strong>{jsonMode ? 'JSON' : formRows.length}</strong><small>{jsonMode ? "计算时校验" : "可直接编辑"}</small></div>
        <div className="task-stat"><span>差异项</span><strong>{result ? differenceCount : '—'}</strong><small>{result ? "基于完整对比结果" : "计算后对比"}</small></div>
        <div className="task-stat"><span>本地已保存</span><strong>{history.length}<small> / 20</small></strong><small>仅保留主动保存的记录</small></div>
      </div>
      <p className="task-scope">
        在本机计算，与已采集运行和验证凭据分开保存。
      </p>
      <div className="local-task-grid">
        <section className="task-panel task-input-panel">
          <div className="task-section-heading"><h2>1. 输入</h2>
          <button
            disabled={busy}
            onClick={() => {
              setInput(JSON.stringify(example, null, 2));
              setFormRows(formRowsFor(example));
              setJsonMode(false);
              setResult(null);
              setMessage("示例已填入，尚未计算。");
            }}
          >
            填入示例
          </button></div>
          <div className="task-actions task-view-switch" role="group" aria-label="输入格式">
            <button
              type="button"
              disabled={busy || !jsonMode}
              aria-pressed={!jsonMode}
              onClick={showForm}
            >
              编辑物品
            </button>
            <button
              type="button"
              disabled={busy || jsonMode}
              aria-pressed={jsonMode}
              onClick={() => setJsonMode(true)}
            >
              编辑 JSON
            </button>
          </div>
          {!jsonMode ? (
            <>
              <div className="quantity-editor">
                {formRows.map((row, index) => (
                  <fieldset key={index} disabled={busy}>
                    <legend>物品 {index + 1}</legend>
                    <label>
                      物品名称
                      <input
                        aria-label={"物品名称 " + (index + 1)}
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
                      应有数量
                      <input
                        aria-label={"应有数量 " + (index + 1)}
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
                      实收数量
                      <input
                        aria-label={"实收数量 " + (index + 1)}
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
                      aria-label={"移除物品 " + (index + 1)}
                      disabled={formRows.length <= 1}
                      onClick={() =>
                        editRows(formRows.filter((_, i) => i !== index))
                      }
                    >
                      移除
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
                添加物品
              </button>
              <p>
                填写物品名称与整数数量；已有任务输入可切换到 JSON。
              </p>
            </>
          ) : null}
          {jsonMode ? (
            <>
              <label htmlFor="task-input">
                应有数量与实收数量（JSON）
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
                    "输入已改变，请重新计算以更新结果。",
                  );
                }}
              />
            </>
          ) : null}
          <p>
            同名物品会合并计数，数量必须为非负整数。
          </p>
          <button className="button button-primary" disabled={busy} onClick={run}>
            {busy ? "正在计算…" : "计算差异"}
          </button>
        </section>
        <section className="task-panel task-result-panel">
          <h2>2. 结果</h2>
          <p role="status">
            {message || "暂无结果，请填入示例或输入数量。"}
          </p>
          {result ? (
            <>
              <div className="task-totals" data-balanced={result.balanced}>
                <span className="task-result-symbol" aria-hidden="true">{result.balanced ? '=' : 'Δ'}</span>
                <strong>{result.rows.length} 种物品</strong>
                <span>
                  {result.rows.filter((r) => r.difference !== 0).length}{" "}
                  项差异
                </span>
                <span>{result.balanced ? "数量一致" : "需要复核"}</span>
              </div>
              <div className="task-table-toolbar"><span>对比明细</span><label className="task-difference-toggle"><input type="checkbox" checked={onlyDifferences} onChange={event => setOnlyDifferences(event.target.checked)} />仅看差异</label></div>
              <div className="task-table">
                <table>
                  <thead>
                    <tr>
                      <th>物品</th>
                      <th>应有数量</th>
                      <th>实收数量</th>
                      <th>差异</th>
                    </tr>
                  </thead>
                  <tbody>
                    {visibleRows.map((r) => (
                      <tr key={r.item} data-difference={r.difference !== 0}>
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
              {onlyDifferences && visibleRows.length === 0 && <p className="task-filter-empty">本次结果没有差异，仍可导出完整结果。</p>}
              <div className="task-actions">
                <button onClick={save} disabled={!!initial.error} data-desktop-save="calculation">
                  {nativeWorkspace() ? "保存到本应用" : "保存到本标签页"}
                </button>
                <button
                  onClick={() => download(result, "reconciliation-result.json")}
                >
                  导出结果
                </button>
                <button
                  onClick={() => download(JSON.parse(input), "request.json")}
                >
                  导出任务输入
                </button>
              </div>
              <details>
                <summary>接入本机采集流程</summary>
                <p>
                  使用导出的 request.json 创建对账任务，核对输入后再执行采集与验证。页面中的数量计算不会生成验证凭据。
                </p>
                <code>
                  skilltape starter new reconcile --request request.json --output NEW_TASK_DIRECTORY
                </code>
              </details>
            </>
          ) : <div className="task-result-empty"><svg viewBox="0 0 120 100" width="120" height="100" aria-hidden="true"><rect x="17" y="10" width="86" height="78" rx="10"/><path d="M17 32h86M46 32v56M74 32v56M17 52h86M17 70h86"/></svg><strong>等待计算</strong><p>计算后，这里会显示数量一致的物品和差异。</p></div>}
        </section>
      </div>
      <section className="task-panel task-history-panel">
        <h2>{nativeWorkspace() ? "3. 本应用保存的记录" : "3. 本标签页保存的记录"}</h2>
        <p>{nativeWorkspace()
          ? "最多20条记录、512 KB。主动保存的记录在移除前持续保留于本应用，与工作区执行证据分开存放。"
          : "最多20条记录、512 KB。仅保留主动保存记录，关闭标签页会清除会话存储。"}</p>
        {history.length ? (
          history.map((item) => (
            <div className="saved-task" key={item.id}>
              <span>
                {new Date(item.at).toLocaleString('zh-CN', { hour12: false })} · {item.result.rows.length}{" "}
                种物品 · {item.result.balanced ? "数量一致" : "存在差异"}
              </span>
              <button
                onClick={() => {
                  if (
                    window.confirm(
                      "使用这条保存记录替换当前输入？",
                    )
                  ) {
                    setInput(JSON.stringify(item.input, null, 2));
                    setFormRows(formRowsFor(item.input));
                    setJsonMode(false);
                    setResult(item.result);
                    setUnsaved(false);
                    setMessage("已打开保存的本机计算。");
                  }
                }}
              >
                打开
              </button>
              <button
                onClick={() => download(item, "saved-reconciliation.json")}
              >
                导出
              </button>
              <button
                onClick={() => {
                  if (window.confirm("移除这条本地保存记录？")) {
                    try {
                      const next = history.filter((n) => n.id !== item.id);
                      savedStorage().setItem(
                        key,
                        JSON.stringify({ version: 1, items: next }),
                      );
                      setHistory(next);
                    } catch {
                      setMessage("无法更新本地历史。");
                    }
                  }
                }}
              >
                移除
              </button>
            </div>
          ))
        ) : (
          <p>暂无保存记录。</p>
        )}
      </section>
    </section>
  );
}
