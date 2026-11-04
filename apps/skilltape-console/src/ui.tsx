import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { getSkills, isSavedEvidenceId } from './api'
import type { Collection } from './types'

export function SkillPicker({ skillId, onOpen }: { skillId: string; onOpen: (id: string) => void }) {
  const [error, setError] = useState(skillId && !isSavedEvidenceId(skillId) ? 'Use a package directory name, not a path.' : '')
  const input = useRef<HTMLInputElement>(null)
  const [catalogOpen, setCatalogOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [offset, setOffset] = useState(0)
  const [reload, setReload] = useState(0)
  const [catalog, setCatalog] = useState<Collection<string> | null>(null)
  const [catalogLoading, setCatalogLoading] = useState(false)
  const [catalogError, setCatalogError] = useState(false)
  useEffect(() => {
    if (!catalogOpen) return
    const controller = new AbortController()
    setCatalogLoading(true); setCatalogError(false); setCatalog(null)
    const timer = window.setTimeout(() => {
      getSkills(controller.signal, query, offset)
        .then(value => { if (!controller.signal.aborted) setCatalog(value) })
        .catch(() => { if (!controller.signal.aborted) setCatalogError(true) })
        .finally(() => { if (!controller.signal.aborted) setCatalogLoading(false) })
    }, 200)
    return () => { window.clearTimeout(timer); controller.abort() }
  }, [catalogOpen, query, offset, reload])
  return (
    <section className="panel receipt-picker" aria-label="Open a saved Skill"><form onSubmit={event => {
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
        <input ref={input} id="skill-id" name="skill-id" defaultValue={skillId} maxLength={255} required
          autoComplete="off" spellCheck={false} onInput={() => setError('')}
          aria-describedby="skill-picker-help skill-picker-error" aria-invalid={error ? true : undefined} />
        <button className="button" type="submit">Open Skill</button>
      </div>
      <p id="skill-picker-help" className="muted">Enter an ID or browse saved Skills below. Opening a Skill does not run it.</p>
      <p id="skill-picker-error" role="alert">{error}</p>
    </form>
    <details className="skill-catalog" open={catalogOpen} onToggle={event => setCatalogOpen(event.currentTarget.open)}>
      <summary>Browse saved Skills</summary>
      {catalogOpen ? <div className="skill-catalog-content">
        <label htmlFor="skill-search">Search saved Skills</label>
        <input id="skill-search" type="search" maxLength={80} autoComplete="off" value={query}
          onChange={event => { setCatalogLoading(true); setQuery(event.target.value); setOffset(0) }} />
        {catalogLoading ? <p role="status">Loading saved Skills…</p> : catalogError ? <>
          <p role="alert">Saved Skills could not be listed. Retry the list, or enter an ID above.</p>
          <button className="button button-secondary" type="button" onClick={() => { setCatalogLoading(true); setReload(value => value + 1) }}>Retry saved Skills</button>
        </> : catalog ? <>
          <p role="status">{catalog.total ? `Showing ${catalog.offset + 1}–${catalog.offset + catalog.items.length} of ${catalog.total} saved Skills` : 'No saved Skill folders match this search.'}</p>
          <ul className="skill-catalog-list" aria-label="Saved Skills">{catalog.items.map(id => <li key={id}>
            <button type="button" onClick={() => { if (input.current) input.current.value = id; setError(''); setCatalogOpen(false); onOpen(id) }}>{id}</button>
          </li>)}</ul>
          <nav className="receipt-picker-controls" aria-label="Saved Skill pages">
            <button className="button button-secondary" type="button" disabled={offset === 0} onClick={() => { setCatalogLoading(true); setOffset(Math.max(0, offset - 50)) }}>Previous saved Skills page</button>
            <button className="button button-secondary" type="button" disabled={catalog.next_offset === null} onClick={() => { setCatalogLoading(true); setOffset(catalog.next_offset ?? offset) }}>Next saved Skills page</button>
          </nav>
          <p className="muted">Names from this workspace. Package contents are checked when opened.</p>
        </> : null}
      </div> : null}
    </details></section>
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
  const [expanded, setExpanded] = useState(open)
  const preview = useMemo(() => {
    if (!expanded) return ''
    const text = JSON.stringify(value, null, 2) ?? 'null'
    return text.length > 65536 ? text.slice(0, 65536) + '\n… Preview truncated. Inspect the saved JSON for the complete record.' : text
  }, [expanded, value])
  return (
    <details className="json-disclosure" open={expanded} onToggle={event => setExpanded(event.currentTarget.open)}>
      <summary>{label}</summary>
      {expanded && <CodeBlock value={preview} />}
    </details>
  )
}

export function titleCase(value: string): string {
  return value
    .replaceAll('_', ' ')
    .replace(/\b\w/g, (character) => character.toUpperCase())
}
