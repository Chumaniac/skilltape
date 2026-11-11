import { useEffect, useState } from 'react'
import { getRunContext, isSavedEvidenceId } from '../api'
import { compareRunContexts, type ContextReview } from '../run-context'
import { StatusBadge } from '../ui'

export function RunConditions({ receiptId }: { receiptId: string }) {
  const [review, setReview] = useState<ContextReview | null>(null)
  const [error, setError] = useState('')
  const [baseline, setBaseline] = useState('')
  const [request, setRequest] = useState<{id: string} | null>(null)
  const [comparison, setComparison] = useState<string[] | null | undefined>(undefined)
  const [comparing, setComparing] = useState(false)
  useEffect(() => {
    const controller = new AbortController()
    setReview(null); setComparing(false); setError(''); setRequest(null); setComparison(undefined); setBaseline('')
    getRunContext(receiptId, controller.signal).then(value => { if (!controller.signal.aborted) setReview(value) })
      .catch(() => { if (!controller.signal.aborted) setError('Run conditions could not be read. The original receipt remains available.') })
    return () => controller.abort()
  }, [receiptId])
  useEffect(() => {
    if (!request) return
    const controller = new AbortController()
    setComparing(true); setComparison(undefined); setError('')
    Promise.all([getRunContext(request.id, controller.signal), getRunContext(receiptId, controller.signal)])
      .then(([before, after]) => { if (!controller.signal.aborted) { setReview(after); setComparison(compareRunContexts(before, after)) } })
      .catch(() => { if (!controller.signal.aborted) setError('The selected conditions could not be compared. Check both saved IDs.') })
      .finally(() => { if (!controller.signal.aborted) setComparing(false) })
    return () => controller.abort()
  }, [request, receiptId])
  const context = review?.context
  return <section className="panel" aria-label="Run conditions">
    <h2>Run conditions</h2>
    {error ? <p role="alert">{error}</p> : !review ? <p role="status">Reading saved conditions…</p> : review.status === 'unrecorded' ? <p>Not recorded for this receipt.</p> : review.status === 'invalid' ? <p role="alert">Context binding failed. Conditions are unavailable.</p> : context ? <>
      <StatusBadge tone="neutral">Recorded · receipt bytes match</StatusBadge>
      <dl className="receipt-facts">
        <div><dt>Input snapshot</dt><dd className="context-digest"><code>{context.input_set_sha256}</code></dd></div>
        <div><dt>Skill package</dt><dd className="context-digest"><code>{context.skill_hash}</code></dd></div>
        <div><dt>Executor</dt><dd>{context.executor_version} · {context.os}/{context.arch}</dd></div>
        <div><dt>Configured limits</dt><dd>{context.max_processes} processes · {context.step_timeout_ms} ms/step · {context.max_output_bytes} output bytes</dd></div>
      </dl>
    </> : null}
    <form className="receipt-picker-controls" onSubmit={event => {
      event.preventDefault()
      const id = baseline.trim().replace(/\.json$/i, '')
      if (!isSavedEvidenceId(id) || id === receiptId) { setError('Choose a different saved receipt filename.'); return }
      setRequest({id})
    }}>
      <label htmlFor="context-baseline">Baseline receipt ID</label>
      <input id="context-baseline" value={baseline} maxLength={255} autoComplete="off" spellCheck={false} required onChange={event => { setBaseline(event.target.value); setError(''); setRequest(null); setComparison(undefined); setComparing(false) }} />
      <button className="button" type="submit" disabled={comparing || !baseline || !review}>Compare conditions</button>
    </form>
    {comparing ? <p role="status">Reading both saved records…</p> : comparison === null ? <p>Comparison unavailable: conditions are missing or their binding failed.</p> : comparison ? comparison.length ? <ul aria-label="Observed condition differences">{comparison.map(item => <li key={item}>{item}</li>)}</ul> : <p>No recorded condition differences.</p> : null}
    <p className="panel-note">Input digest observed by the bounded pre-run scan. Local metadata only; no frozen-input guarantee, author authentication or automatic replay.</p>
  </section>
}
