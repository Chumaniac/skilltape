import type { ReactNode } from 'react'

export function PageLoading({ label = 'Loading…' }: { label?: string }) {
  return (
    <div className="state-card" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      <p>{label}</p>
    </div>
  )
}

export function PageError({
  message,
  onRetry,
}: {
  message: string
  onRetry: () => void
}) {
  return (
    <div className="state-card state-card-error" role="alert">
      <span className="state-icon" aria-hidden="true">
        !
      </span>
      <div>
        <h2>Local API unavailable</h2>
        <p>{message}</p>
        <button className="button button-secondary" type="button" onClick={onRetry}>
          Retry request
        </button>
      </div>
    </div>
  )
}

export function EmptyState({
  title,
  message,
  action,
}: {
  title: string
  message: string
  action?: ReactNode
}) {
  return (
    <div className="state-card state-card-empty">
      <span className="empty-orbit" aria-hidden="true" />
      <div>
        <h2>{title}</h2>
        <p>{message}</p>
        {action}
      </div>
    </div>
  )
}

export function Metric({
  label,
  value,
  tone = 'neutral',
}: {
  label: string
  value: string
  tone?: 'neutral' | 'accent' | 'good' | 'warn' | 'danger'
}) {
  return (
    <div className={'metric metric-' + tone}>
      <span className="metric-label">{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

export function StatusBadge({
  children,
  tone = 'neutral',
}: {
  children: ReactNode
  tone?: 'neutral' | 'good' | 'warn' | 'danger'
}) {
  return <span className={'status-badge status-' + tone}>{children}</span>
}

export function CodeBlock({ value, label }: { value: string; label?: string }) {
  return (
    <div className="code-wrap">
      {label ? <span className="code-label">{label}</span> : null}
      <pre className="code-block">
        <code>{value}</code>
      </pre>
    </div>
  )
}

export function JsonDisclosure({
  label,
  value,
  open = false,
}: {
  label: string
  value: unknown
  open?: boolean
}) {
  return (
    <details className="json-disclosure" open={open}>
      <summary>{label}</summary>
      <CodeBlock value={JSON.stringify(value, null, 2)} />
    </details>
  )
}

export function titleCase(value: string): string {
  return value
    .replaceAll('_', ' ')
    .replace(/\b\w/g, (character) => character.toUpperCase())
}
