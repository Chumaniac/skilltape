import { useEffect, useState } from 'react'

import { formatNumber, getReceipt } from '../api'
import type { Receipt } from '../types'
import { CodeBlock, EmptyState, JsonDisclosure, PageError, PageLoading, StatusBadge, titleCase } from '../ui'

interface ReceiptViewProps {
  receiptId?: string
}

function ReceiptPicker({ receiptId, onOpen }: { receiptId: string; onOpen: (id: string) => void }) {
  const [error, setError] = useState('')
  return (
    <form className="panel receipt-picker" onSubmit={(event) => {
      event.preventDefault()
      const id = String(new FormData(event.currentTarget).get('receipt-id') ?? '').trim().replace(/\.json$/i, '')
      if (!id || id === '.' || id === '..' || /[/\\:\u0000-\u001f\u007f]/.test(id)) {
        setError('Use a saved filename, not a path.')
        return
      }
      setError('')
      onOpen(id)
    }}>
      <label htmlFor="receipt-id">Saved Receipt ID</label>
      <div className="receipt-picker-controls">
        <input id="receipt-id" name="receipt-id" defaultValue={receiptId} maxLength={255} required
          aria-describedby="receipt-picker-help receipt-picker-error" aria-invalid={error ? true : undefined}
          onInput={() => setError('')} autoComplete="off" spellCheck={false} />
        <button className="button" type="submit">Open Receipt</button>
      </div>
      <p id="receipt-picker-help" className="muted">Enter a filename from .skilltape/receipts, with or without .json. This viewer only reads saved evidence.</p>
      <p id="receipt-picker-error" role="alert">{error}</p>
    </form>
  )
}

export function ReceiptViewPage({ receiptId = 'run-a' }: ReceiptViewProps) {
  const [receipt, setReceipt] = useState<Receipt | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)
  const [failedOnly, setFailedOnly] = useState(false)

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setError('')
    getReceipt(receiptId, controller.signal)
      .then((stored) => { if (!controller.signal.aborted) setReceipt(stored.document) })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : 'Could not read the verification Receipt.')
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [receiptId, reload])

  const picker = <ReceiptPicker key={receiptId} receiptId={receiptId} onOpen={(id) => {
    if (id === receiptId) setReload(value => value + 1)
    else window.location.hash = 'receipt?run=' + encodeURIComponent(id)
  }} />
  if (loading) return <>{picker}<PageLoading label="Loading verification Receipt…" /></>
  if (error) return <>{picker}<PageError message={error} onRetry={() => setReload((value) => value + 1)} /></>
  if (!receipt) {
    return <>{picker}<EmptyState title="No Receipt selected" message="Run skilltape verify to create a Receipt for this view." /></>
  }

  const failed = receipt.assertions.filter(assertion => !assertion.passed)
  const visible = failedOnly ? failed : receipt.assertions
  const statusTone = receipt.status === 'succeeded' ? 'good' : receipt.status === 'assertion_failed' ? 'warn' : 'danger'
  return (
    <>{picker}<section className="page-section receipt-review">
      <header className="page-heading">
        <div>
          <p className="eyebrow">Verify run</p>
          <h1>Evidence you can inspect, not just trust</h1>
          <p className="page-description">
            This Receipt contains hashes and bounded summaries instead of raw command output or secrets.
          </p>
        </div>
        <StatusBadge tone={statusTone}>{titleCase(receipt.status)}</StatusBadge>
      </header>

      <div className="metric-grid">
        <div className="metric metric-accent">
          <span className="metric-label">Run ID</span>
          <strong>{receipt.run_id}</strong>
        </div>
        <div className="metric">
          <span className="metric-label">Steps</span>
          <strong>{formatNumber(receipt.steps.length)}</strong>
        </div>
        <div className="metric">
          <span className="metric-label">Assertions</span>
          <strong>{formatNumber(receipt.assertions.length)}</strong>
        </div>
        <div className="metric">
          <span className="metric-label">Policy decisions</span>
          <strong>{formatNumber(receipt.policy_decisions.length)}</strong>
        </div>
      </div>

      <div className="review-grid">
        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Execution trace</p>
              <h2>Step status</h2>
            </div>
            <span className="panel-note">Output redacted</span>
          </div>
          {receipt.steps.length > 0 ? (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th scope="col">Step</th>
                    <th scope="col">Status</th>
                    <th scope="col">Exit</th>
                    <th scope="col">Stdout</th>
                    <th scope="col">Stderr</th>
                  </tr>
                </thead>
                <tbody>
                  {receipt.steps.map((step) => (
                    <tr key={step.step_id}>
                      <th scope="row"><code>{step.step_id}</code></th>
                      <td><StatusBadge tone={step.status === 'succeeded' ? 'good' : 'danger'}>{titleCase(step.status)}</StatusBadge></td>
                      <td>{step.exit_code === null ? '—' : step.exit_code}</td>
                      <td><code>{step.stdout_sha256.slice(0, 10)}…</code><span className="table-sub">{formatNumber(step.stdout_bytes)} bytes</span></td>
                      <td><code>{step.stderr_sha256.slice(0, 10)}…</code><span className="table-sub">{formatNumber(step.stderr_bytes)} bytes</span></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <EmptyState title="No step summaries" message="The run did not persist any step-level evidence." />
          )}
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Assertions</p>
              <h2>Expected outcomes</h2>
            </div>
          </div>
          <p role="status">{receipt.assertions.length - failed.length} passed · {failed.length} failed</p>
          <div className="receipt-filters" role="group" aria-label="Assertion filters">
            <button className="button button-secondary" type="button" aria-pressed={!failedOnly} onClick={() => setFailedOnly(false)}>All checks ({receipt.assertions.length})</button>
            <button className="button button-secondary" type="button" aria-pressed={failedOnly} onClick={() => setFailedOnly(true)}>Needs attention ({failed.length})</button>
          </div>
          {receipt.assertions.length > 0 ? (
            visible.length > 0 ? <ul className="result-list">
              {visible.map((assertion, index) => (
                <li key={assertion.target + index}>
                  <span className={assertion.passed ? 'result-icon result-pass' : 'result-icon result-fail'} aria-hidden="true">
                    {assertion.passed ? '✓' : '!'}
                  </span>
                  <div>
                    <strong>{titleCase(assertion.kind)}</strong>
                    <StatusBadge tone={assertion.passed ? 'good' : 'danger'}>{assertion.passed ? 'Passed' : 'Failed'}</StatusBadge>
                    <span>{assertion.target}</span>
                    <small>{assertion.reason}</small>
                  </div>
                </li>
              ))}
            </ul> : <p className="muted">No failed assertions. Review the run status and policy decisions separately.</p>
          ) : (
            <p className="muted">No explicit assertions were recorded.</p>
          )}
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Policy</p>
              <h2>Decisions</h2>
            </div>
          </div>
          {receipt.policy_decisions.length > 0 ? (
            <ul className="decision-list">
              {receipt.policy_decisions.map((decision, index) => (
                <li key={decision.step_id + decision.phase + index}>
                  <StatusBadge tone={decision.allowed ? 'good' : 'danger'}>{decision.allowed ? 'allowed' : 'denied'}</StatusBadge>
                  <div>
                    <strong>{decision.step_id}</strong>
                    <span>{decision.code} · {decision.risk}</span>
                    <small>{decision.reason}</small>
                  </div>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">No policy decisions were recorded.</p>
          )}
        </article>

        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Integrity</p>
              <h2>Skill package hash</h2>
            </div>
            <span className="panel-note">SHA-256</span>
          </div>
          <CodeBlock value={receipt.skill_hash} label="skill_hash" />
          <JsonDisclosure label="View Receipt JSON" value={receipt} />
        </article>
      </div>
    </section></>
  )
}
