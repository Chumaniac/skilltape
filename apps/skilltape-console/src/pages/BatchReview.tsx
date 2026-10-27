import { useEffect, useRef, useState } from 'react'
import { ApiError, formatNumber, getBatchReview } from '../api'
import type { BatchCase, BatchReview, BatchStatus } from '../batch'
import { CodeBlock, EmptyState, PageError, PageLoading, StatusBadge, titleCase } from '../ui'

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
      else setError(reason instanceof Error ? reason.message : 'Batch progress could not be read.')
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
    <header className="page-heading"><div><h1>Review a verification batch</h1><p className="page-description">Choose an input case, inspect its saved evidence, then decide the next CLI step.</p></div>
      <button className="button button-secondary" type="button" disabled={loading} onClick={() => setRevision(value => value + 1)}>Refresh batch</button>
    </header>
    {loading ? <PageLoading label="Reading batch progress…" /> : error ? <PageError message={error} onRetry={() => setRevision(value => value + 1)} /> : missing ?
      <EmptyState title="No batch workspace found" message="Prepare a batch with the source CLI, then open Console with that batch workspace." /> : report ? <>
      <div className="batch-overview"><div><h2>Reported batch: {titleCase(report.status)}</h2><p>Receipt bindings are checked here. Open a delivery to check physical files.</p></div>
        <dl className="receipt-facts"><div><dt>cases</dt><dd>{formatNumber(report.cases.length)}</dd></div><div><dt>succeeded</dt><dd>{formatNumber(succeeded)}</dd></div><div><dt>need attention</dt><dd>{formatNumber(report.cases.length - succeeded)}</dd></div></dl>
      </div>
      <div className="batch-workspace">
        <article className="panel batch-queue" aria-label="Batch case queue">
          <div className="panel-heading"><h2>Input cases</h2><span className="panel-note">Choose a case to review</span></div>
          <label className="batch-attention-filter"><input name="attention" type="checkbox" checked={attention} onChange={event => setAttention(event.target.checked)} />Only cases needing attention</label>
          <table className="batch-case-table"><thead><tr><th scope="col">Case</th><th scope="col">Reported outcome</th><th scope="col">Review</th></tr></thead><tbody>
            {visible.map(row => <tr key={row.id} data-batch-case data-selected={selected?.id === row.id || undefined}>
              <th scope="row"><code>{row.id}</code><span className="table-sub">{formatNumber(row.attempt_count)} attempt{row.attempt_count === 1 ? '' : 's'}</span></th>
              <td><StatusBadge tone={tone(row.status)}>{titleCase(row.status)}</StatusBadge></td>
              <td><button className="button button-secondary" type="button" aria-label={'Review case ' + row.id} aria-pressed={selected?.id === row.id} onClick={() => {
                if (selected?.id === row.id) detail.current?.querySelector<HTMLElement>('h2')?.focus()
                else { focusRequested.current = true; setSelectedId(row.id) }
              }}>Review</button></td>
            </tr>)}
            {!visible.length ? <tr><td colSpan={3}>No cases need attention in this recorded batch. Clear the filter to review successful cases.</td></tr> : null}
          </tbody></table>
        </article>
        <div ref={detail}>{selected ? <CaseDetails key={selected.id} row={selected} /> : <EmptyState title="No case selected" message="Clear the attention filter to review the retained successful evidence." />}</div>
      </div>
      <details className="panel batch-bindings"><summary>Material bindings and review limits</summary><dl><dt>Recorded revision</dt><dd>{formatNumber(report.revision)}</dd><dt>Skill</dt><dd><code>{report.package_sha256}</code></dd><dt>Plan</dt><dd><code>{report.plan_sha256}</code></dd></dl>
        <p>Up to {report.max_cases} cases and {report.max_attempts} attempts per case. Local hashes do not authenticate authors or establish business acceptance. Console never executes the commands shown here.</p>
      </details>
    </> : null}
  </section>
}

function CaseDetails({ row }: { row: BatchCase }) {
  const changed = row.evidence === 'changed'
  const retry = !changed && (row.status === 'failed' || row.status === 'cancelled')
  const command = changed ? 'skilltape batch inspect <batch-workspace> --json' : row.status === 'pending' ? 'skilltape batch run <batch-workspace> --yes' : retry ? `skilltape batch retry <batch-workspace> --case ${row.id} --input <corrected-input> --yes` : row.status === 'running' || row.status === 'unconfirmed' ? 'skilltape batch reconcile <batch-workspace>' : null
  const guidance = changed ? 'Saved evidence changed or is missing. Inspect the originals before continuing.' : row.status === 'succeeded' ? 'This attempt is retained. Open its delivery to compare the physical files with the saved manifest.' : retry ? 'Keep this attempt. Review the failure or cancellation, then explicitly retry with corrected inputs when needed.' : row.status === 'pending' ? 'This case has not run. The CLI runs pending cases in plan order and preserves successful attempts.' : 'The outcome is not confirmed. Inspect and reconcile retained evidence before deciding what to do; this case will not retry automatically.'
  return <article className="panel batch-case-detail" aria-label={'Selected case ' + row.id}>
    <div className="panel-heading"><h2 tabIndex={-1}><code>{row.id}</code></h2><StatusBadge tone={tone(row.status)}>{titleCase(row.status)}</StatusBadge></div>
    <p className="case-guidance">{guidance}</p>
    {row.error_code ? <p className="muted">Recorded error: <code>{row.error_code}</code></p> : null}
    <div className="case-evidence-actions">
      {row.receipt_id ? <a className="button" aria-label={'Open Receipt for ' + row.id} href={receiptHref(row.receipt_id)}>Open Receipt</a> : <p className="muted">No completed Receipt recorded</p>}
      {row.delivery_id ? <a className="button button-secondary" aria-label={'Inspect delivery for ' + row.id} href={deliveryHref(row.delivery_id)}>Inspect delivery</a> : null}
    </div>
    {command ? <CodeBlock label="Next CLI step · replace local placeholders" value={command} /> : null}
    {row.attempts.length > 1 ? <details className="case-attempts"><summary>Earlier attempts</summary><ol>{row.attempts.slice(0, -1).map(attempt => <li key={attempt.number}>Attempt {attempt.number}: {titleCase(attempt.status)} · {attempt.receipt_id ? <a aria-label={`Receipt for ${row.id} attempt ${attempt.number}`} href={receiptHref(attempt.receipt_id)}>Receipt</a> : 'No completed evidence'}</li>)}</ol></details> : null}
    <details className="case-input-binding"><summary>Frozen input binding</summary><code>{row.input_sha256}</code></details>
  </article>
}
