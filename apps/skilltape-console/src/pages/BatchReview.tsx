import { useEffect, useRef, useState } from 'react'
import { ApiError, formatNumber, getBatchReview } from '../api'
import type { BatchCase, BatchReview, BatchStatus } from '../batch'
import { CodeBlock, EmptyState, PageError, PageLoading, StatusBadge, evidenceLabel } from '../ui'

const tone = (status: BatchStatus) => status === 'succeeded' ? 'good' : status === 'failed' || status === 'unconfirmed' ? 'danger' : status === 'cancelled' ? 'warn' : 'neutral'
const receiptHref = (id: string) => '#receipt?run=' + encodeURIComponent(id)
const deliveryHref = (id: string) => '#deliveries?delivery=' + encodeURIComponent(id)

export function BatchReviewPage() {
  const [report, setReport] = useState<BatchReview | null>(null)
  const [loading, setLoading] = useState(true)
  const [missing, setMissing] = useState(false)
  const [error, setError] = useState('')
  const [revision, setRevision] = useState(0)
  const [attention, setAttention] = useState(false)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const detail = useRef<HTMLDivElement>(null)
  const focusRequested = useRef(false)

  useEffect(() => {
    const controller = new AbortController()
    setReport(null); setError(''); setMissing(false); setLoading(true)
    getBatchReview(controller.signal).then(value => {
      if (!controller.signal.aborted) setReport(value)
    }).catch(reason => {
      if (controller.signal.aborted) return
      if (reason instanceof ApiError && reason.status === 404) setMissing(true)
      else setError(reason instanceof Error ? reason.message : "无法读取批次进度。")
    }).finally(() => { if (!controller.signal.aborted) setLoading(false) })
    return () => controller.abort()
  }, [revision])

  const visible = report?.cases.filter(row => !attention || row.status !== 'succeeded') ?? []
  const selected = visible.find(row => row.id === selectedId) ?? visible.find(row => row.status !== 'succeeded') ?? visible[0]
  const succeeded = report?.cases.filter(row => row.status === 'succeeded').length ?? 0
  useEffect(() => {
    if (focusRequested.current && selected) {
      focusRequested.current = false
      detail.current?.querySelector<HTMLElement>('h2')?.focus()
    }
  }, [selected?.id])

  return <section className="page-section batch-workbench">
    <header className="page-heading"><div><h1>复核验证批次</h1><p className="page-description">选择输入任务，查看已保存证据，再确定下一条 CLI 命令。</p></div>
      <button className="button button-secondary" type="button" disabled={loading} onClick={() => setRevision(value => value + 1)}>刷新批次</button>
    </header>
    {loading ? <PageLoading label="正在读取批次进度…" /> : error ? <PageError message={error} onRetry={() => setRevision(value => value + 1)} /> : missing ?
      <EmptyState title="未找到批次工作区" message="用源码 CLI 准备批次，再在控制台打开该批次工作区。" /> : report ? <>
      <div className="batch-overview"><div><h2>批次记录： {evidenceLabel(report.status)}</h2><p>这里检查凭据绑定关系；检查实际文件请打开交付包。</p></div>
        <dl className="receipt-facts"><div><dt>个任务</dt><dd>{formatNumber(report.cases.length)}</dd></div><div><dt>成功</dt><dd>{formatNumber(succeeded)}</dd></div><div><dt>项需处理</dt><dd>{formatNumber(report.cases.length - succeeded)}</dd></div></dl>
      </div>
      <div className="batch-workspace">
        <article className="panel batch-queue" aria-label="批次任务队列">
          <div className="panel-heading"><h2>输入任务</h2><span className="panel-note">选择要复核的任务</span></div>
          <label className="batch-attention-filter"><input name="attention" type="checkbox" checked={attention} onChange={event => setAttention(event.target.checked)} />仅需处理的任务</label>
          <table className="batch-case-table"><thead><tr><th scope="col">任务</th><th scope="col">记录的结果</th><th scope="col">复核</th></tr></thead><tbody>
            {visible.map(row => <tr key={row.id} data-batch-case data-selected={selected?.id === row.id || undefined}>
              <th scope="row"><code>{row.id}</code><span className="table-sub">{formatNumber(row.attempt_count)} 次尝试{row.attempt_count === 1 ? '' : 's'}</span></th>
              <td><StatusBadge tone={tone(row.status)}>{evidenceLabel(row.status)}</StatusBadge></td>
              <td><button className="button button-secondary" type="button" aria-label={"复核任务 " + row.id} aria-pressed={selected?.id === row.id} onClick={() => {
                if (selected?.id === row.id) detail.current?.querySelector<HTMLElement>('h2')?.focus()
                else { focusRequested.current = true; setSelectedId(row.id) }
              }}>复核</button></td>
            </tr>)}
            {!visible.length ? <tr><td colSpan={3}>此批次没有待处理任务，清除筛选可查看成功任务。</td></tr> : null}
          </tbody></table>
        </article>
        <div ref={detail}>{selected ? <CaseDetails key={selected.id} row={selected} /> : <EmptyState title="尚未选择任务" message="清除待处理筛选，即可查看保留的成功证据。" />}</div>
      </div>
      <details className="panel batch-bindings"><summary>材料绑定与复核范围</summary><dl><dt>记录的版本</dt><dd>{formatNumber(report.revision)}</dd><dt>技能包</dt><dd><code>{report.package_sha256}</code></dd><dt>计划</dt><dd><code>{report.plan_sha256}</code></dd></dl>
        <p>最多 {report.max_cases} 个任务和 {report.max_attempts} 次尝试。本机哈希不认证作者身份，也不代表业务验收；控制台不会执行这里展示的命令。</p>
      </details>
    </> : null}
  </section>
}

function CaseDetails({ row }: { row: BatchCase }) {
  const changed = row.evidence === 'changed'
  const retry = !changed && (row.status === 'failed' || row.status === 'cancelled')
  const command = changed ? 'skilltape batch inspect <batch-workspace> --json' : row.status === 'pending' ? 'skilltape batch run <batch-workspace> --yes' : retry ? `skilltape batch retry <batch-workspace> --case ${row.id} --input <corrected-input> --yes` : row.status === 'running' || row.status === 'unconfirmed' ? 'skilltape batch reconcile <batch-workspace>' : null
  const guidance = changed ? "已保存证据发生变化或缺失，继续前请检查原件。" : row.status === 'succeeded' ? "本次尝试已保留，可打开其交付包，将实际文件与已保存清单对比。" : retry ? "保留本次尝试，先复核失败或取消原因；需要时再明确使用修正后的输入重试。" : row.status === 'pending' ? "此任务尚未运行。CLI 会按计划顺序处理待执行任务，并保留成功尝试。" : "结果尚未确认，请先核对保留证据再决定下一步；此任务不会自动重试。"
  return <article className="panel batch-case-detail" aria-label={"当前任务 " + row.id}>
    <div className="panel-heading"><h2 tabIndex={-1}><code>{row.id}</code></h2><StatusBadge tone={tone(row.status)}>{evidenceLabel(row.status)}</StatusBadge></div>
    <p className="case-guidance">{guidance}</p>
    {row.error_code ? <p className="muted">已记录错误： <code>{row.error_code}</code></p> : null}
    <div className="case-evidence-actions">
      {row.receipt_id ? <a className="button" aria-label={"打开任务凭据 " + row.id} href={receiptHref(row.receipt_id)}>打开凭据</a> : <p className="muted">未记录完成凭据</p>}
      {row.delivery_id ? <a className="button button-secondary" aria-label={"检查交付包 " + row.id} href={deliveryHref(row.delivery_id)}>检查交付包</a> : null}
    </div>
    {command ? <CodeBlock label="下一条 CLI 命令 · 请替换本机占位值" value={command} /> : null}
    {row.attempts.length > 1 ? <details className="case-attempts"><summary>此前尝试</summary><ol>{row.attempts.slice(0, -1).map(attempt => <li key={attempt.number}>尝试 {attempt.number}: {evidenceLabel(attempt.status)} · {attempt.receipt_id ? <a aria-label={`任务凭据 ${row.id} 次尝试 ${attempt.number}`} href={receiptHref(attempt.receipt_id)}>验证凭据</a> : "没有已完成证据"}</li>)}</ol></details> : null}
    <details className="case-input-binding"><summary>冻结输入绑定</summary><code>{row.input_sha256}</code></details>
  </article>
}
