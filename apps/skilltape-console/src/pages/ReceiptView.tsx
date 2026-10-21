import { RunConditions } from './RunConditions'

import { useEffect, useState } from 'react'

import { formatNumber, getReceipt, isSavedEvidenceId } from '../api'
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
      if (!isSavedEvidenceId(id)) {
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
      <p id="receipt-picker-help" className="muted">Use a filename from .skilltape/receipts; .json is optional.</p>
      <p id="receipt-picker-error" role="alert">{error}</p>
    </form>
  )
}

export function ReceiptViewPage({ receiptId = '' }: ReceiptViewProps) {
  const [receipt, setReceipt] = useState<Receipt | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)
  const [failedOnly, setFailedOnly] = useState(false)

  useEffect(() => {
    if (!receiptId) { setLoading(false); return }
    if (!isSavedEvidenceId(receiptId)) {
      setError('Use a saved filename, not a path.')
      setLoading(false)
      return
    }
    const controller = new AbortController()
    setLoading(true)
    setError('')
    setFailedOnly(false)
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
  const reviewChecks = failed.length > 0 || (receipt.status === 'succeeded' && receipt.assertions.length > 0)
  const nextStep = failed.length > 0
    ? { title: `${formatNumber(failed.length)} ${failed.length === 1 ? 'check needs' : 'checks need'} attention`,
      detail: 'Compare the failed targets with the expected outputs before verifying again.', action: 'Review failed checks' }
    : receipt.status === 'succeeded' && receipt.assertions.length > 0
      ? { title: 'This run completed', detail: 'Review the saved checks and permissions for this Skill version.', action: 'Review saved checks' }
      : receipt.status === 'succeeded'
        ? { title: 'Run completed without explicit checks', detail: 'Review the execution and policy decisions before reusing this Skill.', action: 'Review step results' }
      : { title: 'Review the execution result', detail: 'Read the step outcomes and policy decisions before changing the Skill.', action: 'Review step results' }
  function openReview() {
    setFailedOnly(failed.length > 0)
    const heading = document.getElementById(reviewChecks ? 'receipt-checks-heading' : 'receipt-steps-heading')
    heading?.focus({ preventScroll: true })
    heading?.scrollIntoView({ block: 'start' })
  }
  return (
    <>{picker}<section className="page-section receipt-review">
      <header className="page-heading">
        <div>
          <p className="eyebrow">Saved run</p>
          <h1>Verification receipt</h1>
          <p className="page-description">
            Step outcomes, output checks and policy decisions recorded for this run.
          </p>
        </div>
        <StatusBadge tone={statusTone}>{titleCase(receipt.status)}</StatusBadge>
      </header>

      <section className="receipt-verdict" aria-label="What to review next">
        <div><h2>{nextStep.title}</h2><p>{nextStep.detail}</p></div>
        <button className="button" type="button" onClick={openReview}>{nextStep.action}</button>
      </section>
      <div className="receipt-run-details">
        <p className="receipt-run-id"><span>Run ID</span><code>{receipt.run_id}</code></p>
        <dl className="receipt-facts">
          <div><dt>Steps</dt><dd>{formatNumber(receipt.steps.length)}</dd></div>
          <div><dt>Checks</dt><dd>{formatNumber(receipt.assertions.length)}</dd></div>
          <div><dt>Policy decisions</dt><dd>{formatNumber(receipt.policy_decisions.length)}</dd></div>
        </dl>
      </div>

      <RunConditions receiptId={receiptId} />
      <div className="review-grid">
        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Execution trace</p>
              <h2 id="receipt-steps-heading" tabIndex={-1}>Step outcomes</h2>
            </div>
            <span className="panel-note">Output digests and byte counts</span>
          </div>
          {receipt.steps.length > 0 ? (
            <div className="table-wrap" role="region" aria-labelledby="receipt-steps-heading" tabIndex={0}>
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
              <h2 id="receipt-checks-heading" tabIndex={-1}>Checks against expected results</h2>
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
