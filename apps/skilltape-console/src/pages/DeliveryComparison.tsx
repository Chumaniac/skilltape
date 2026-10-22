import { useEffect, useState } from 'react'
import { formatNumber, getDelivery } from '../api'
import { compareDeliveries } from '../delivery-comparison'
import type { DeliverySummary } from '../types'
import { PageError, PageLoading, StatusBadge } from '../ui'

export function DeliveryComparison({ items }: { items: DeliverySummary[] }) {
  const [before, setBefore] = useState('')
  const [after, setAfter] = useState('')
  const [request, setRequest] = useState<{ before: string; after: string } | null>(null)
  const [result, setResult] = useState<ReturnType<typeof compareDeliveries>>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const [inspected, setInspected] = useState(false)

  useEffect(() => {
    const controller = new AbortController()
    setResult(null); setError(''); setInspected(false); setLoading(Boolean(request))
    if (request) {
      Promise.all([getDelivery(request.before, controller.signal), getDelivery(request.after, controller.signal)])
        .then(([a, b]) => { if (!controller.signal.aborted) { setResult(compareDeliveries(a, b)); setInspected(true) } })
        .catch(reason => {
          if (!controller.signal.aborted) {
            setError(reason instanceof Error ? reason.message : 'The deliveries could not be compared.')
            setLoading(false); controller.abort()
          }
        })
        .finally(() => { if (!controller.signal.aborted) setLoading(false) })
    }
    return () => controller.abort()
  }, [request])

  const choose = (value: string, baseline: boolean) => {
    setRequest(null); setResult(null); setInspected(false); setError('')
    if (baseline) setBefore(value); else setAfter(value)
  }
  return <article className="panel delivery-comparison" aria-live="polite">
    <div className="panel-heading"><div><h2>Compare saved deliveries</h2></div></div>
    <p>Choose two saved bundles to compare file changes. Both are rechecked.</p>
    <div className="delivery-comparison-controls">
      <label>Baseline delivery<select value={before} onChange={event => choose(event.target.value, true)}>
        <option value="">Choose baseline…</option>{items.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}
      </select></label>
      <label>Replacement delivery<select value={after} onChange={event => choose(event.target.value, false)}>
        <option value="">Choose replacement…</option>{items.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}
      </select></label>
      <button className="button" type="button" disabled={loading || !before || !after || before === after}
        onClick={() => setRequest({ before, after })}>Compare file changes</button>
    </div>
    {loading ? <PageLoading label="Rechecking both saved bundles…" /> : error ? <PageError message={error} onRetry={() => setRequest({ before, after })} /> : inspected ? result ? <>
      <StatusBadge tone="neutral">File comparison complete</StatusBadge>
      <p>{result.counts.added} added · {result.counts.removed} removed · {result.counts.changed} changed · {result.counts.unchanged} unchanged</p>
      <div className="delivery-table-wrap" tabIndex={0} aria-label="Saved delivery file changes"><table className="delivery-table">
        <thead><tr><th>File</th><th>Change</th><th>Baseline bytes</th><th>Replacement bytes</th><th>Baseline SHA-256</th><th>Replacement SHA-256</th></tr></thead>
        <tbody>{result.changes.map(row => <tr key={row.path}><td><code>{row.path}</code></td><td>{row.kind}</td>
          <td>{row.before ? formatNumber(row.before.bytes) : 'Absent'}</td><td>{row.after ? formatNumber(row.after.bytes) : 'Absent'}</td>
          <td><code>{row.before?.sha256 ?? 'Absent'}</code></td><td><code>{row.after?.sha256 ?? 'Absent'}</code></td></tr>)}
          {!result.changes.length ? <tr><td colSpan={6}>No file content or size changes observed.</td></tr> : null}</tbody>
      </table></div>
      {result.truncated ? <p>Counts cover both bundles; only the first 100 changed rows are shown.</p> : null}
    </> : <><StatusBadge tone="warn">Comparison unavailable</StatusBadge><p>Physical evidence is incomplete or failed. Recheck the original bundles before comparing.</p></> : null}
    <details className="panel-note"><summary>Comparison limits</summary><p>Both bundles need complete matching inspections, with at most 100 files each. Comparing local snapshots does not execute content or authenticate authors. Review business requirements separately.</p></details>
  </article>
}
