import { useState, type ReactNode } from 'react'
import { isSavedEvidenceId } from './api'

export function SkillPicker({ skillId, onOpen }: { skillId: string; onOpen: (id: string) => void }) {
  const [error, setError] = useState(skillId && !isSavedEvidenceId(skillId) ? 'Use a package directory name, not a path.' : '')
  return (
    <form className="panel receipt-picker" onSubmit={event => {
      event.preventDefault()
      const id = String(new FormData(event.currentTarget).get('skill-id') ?? '').trim()
      if (!isSavedEvidenceId(id)) {
        setError('Use a package directory name, not a path.')
        return
      }
      setError('')
      onOpen(id)
    }}>
      <label htmlFor="skill-id">Saved Skill ID</label>
      <div className="receipt-picker-controls">
        <input id="skill-id" name="skill-id" defaultValue={skillId} maxLength={255} required
          autoComplete="off" spellCheck={false} onInput={() => setError('')}
          aria-describedby="skill-picker-help skill-picker-error" aria-invalid={error ? true : undefined} />
        <button className="button" type="submit">Open Skill</button>
      </div>
      <p id="skill-picker-help" className="muted">Enter a name from skills/. Opening it does not run the Skill.</p>
      <p id="skill-picker-error" role="alert">{error}</p>
    </form>
  )
}

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
        <h2>Could not load this view</h2>
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
