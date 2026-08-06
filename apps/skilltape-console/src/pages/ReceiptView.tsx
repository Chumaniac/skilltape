import { useEffect, useState } from 'react'

import { formatNumber, getReceipt } from '../api'
import type { Receipt } from '../types'
import { CodeBlock, EmptyState, JsonDisclosure, PageError, PageLoading, StatusBadge, titleCase } from '../ui'

interface ReceiptViewProps {
  receiptId?: string
}

export function ReceiptViewPage({ receiptId = 'run-a' }: ReceiptViewProps) {
  const [receipt, setReceipt] = useState<Receipt | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setError('')
    getReceipt(receiptId, controller.signal)
      .then((stored) => setReceipt(stored.document as unknown as Receipt))
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

  if (loading) return <PageLoading label="Loading verification Receipt…" />
  if (error) return <PageError message={error} onRetry={() => setReload((value) => value + 1)} />
  if (!receipt) {
    return <EmptyState title="No Receipt selected" message="Run skilltape verify to create a Receipt for this view." />
  }

  const statusTone = receipt.status === 'succeeded' ? 'good' : receipt.status === 'assertion_failed' ? 'warn' : 'danger'
  return (
    <section className="page-section">
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
          {receipt.assertions.length > 0 ? (
            <ul className="result-list">
              {receipt.assertions.map((assertion, index) => (
                <li key={assertion.target + index}>
                  <span className={assertion.passed ? 'result-icon result-pass' : 'result-icon result-fail'} aria-hidden="true">
                    {assertion.passed ? '✓' : '!'}
                  </span>
                  <div>
                    <strong>{titleCase(assertion.type)}</strong>
                    <span>{assertion.target}</span>
                    <small>{assertion.reason}</small>
                  </div>
                </li>
              ))}
            </ul>
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
    </section>
  )
}
