import { useEffect, useState } from 'react'

import { formatDate, formatEventPayload, formatNumber, getTapeEvents, getTapes, getWorkspaces } from '../api'
import type { Collection, TapeEvent, TapeEvents, TapeSummary, WorkspaceSummary } from '../types'
import { EmptyState, PageError, PageLoading, titleCase } from '../ui'

interface TimelineProps {
  tapeId?: string
}

interface TimelineData {
  workspace: WorkspaceSummary | null
  tapes: Collection<TapeSummary>
  events: TapeEvents | null
}

export function TimelinePage({ tapeId }: TimelineProps) {
  const [data, setData] = useState<TimelineData | null>(null)
  const [error, setError] = useState('Could not load the local capture timeline.')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setError('')
    Promise.all([getWorkspaces(controller.signal), getTapes('default', controller.signal)])
      .then(async ([workspaces, tapes]) => {
        const selectedTape = tapeId ?? tapes.items[0]?.id
        const events = selectedTape
          ? await getTapeEvents(selectedTape, controller.signal)
          : null
        setData({
          workspace: workspaces.items[0] ?? null,
          tapes,
          events,
        })
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : 'Could not read the local API.')
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [reload, tapeId])

  if (loading) return <PageLoading label="Loading capture timeline…" />
  if (error) return <PageError message={error} onRetry={() => setReload((value) => value + 1)} />
  if (!data || data.tapes.items.length === 0) {
    return (
      <PageSection
        eyebrow="Capture timeline"
        title="See the work as it happened"
        description="Recorded terminal, file, and permission events appear here after a capture."
      >
        <EmptyState
          title="No captures yet"
          message="Run skilltape capture to create the first redacted Tape in this workspace."
        />
      </PageSection>
    )
  }

  const selectedTape = data.events?.tape_id ?? data.tapes.items[0].id
  return (
    <PageSection
      eyebrow="Capture timeline"
      title="See the work as it happened"
      description="Every event stays linked to its source and redaction state."
      actions={
        <label className="select-control">
          <span>Tape</span>
          <select
            aria-label="Select capture tape"
            name="tape"
            value={selectedTape}
            onChange={(event) => {
              window.location.hash = '#timeline?tape=' + encodeURIComponent(event.target.value)
            }}
          >
            {data.tapes.items.map((tape) => (
              <option key={tape.id} value={tape.id}>
                {tape.id}
              </option>
            ))}
          </select>
        </label>
      }
    >
      <div className="metric-grid">
        <div className="metric metric-accent">
          <span className="metric-label">Workspace</span>
          <strong>{data.workspace?.name ?? 'Local workspace'}</strong>
        </div>
        <div className="metric">
          <span className="metric-label">Events</span>
          <strong>{formatNumber(data.events?.total ?? 0)}</strong>
        </div>
        <div className="metric">
          <span className="metric-label">Tape status</span>
          <strong>{data.tapes.items.find((tape) => tape.id === selectedTape)?.finished_at_ms ? 'Finished' : 'Open'}</strong>
        </div>
      </div>
      {data.events && data.events.events.length > 0 ? (
        <ol className="event-list" aria-label="Capture events">
          {data.events.events.map((event) => (
            <TimelineEvent event={event} key={event.sequence} />
          ))}
        </ol>
      ) : (
        <EmptyState
          title="This Tape is empty"
          message="The capture exists, but no events have been persisted yet."
        />
      )}
    </PageSection>
  )
}

function PageSection({
  eyebrow,
  title,
  description,
  actions,
  children,
}: {
  eyebrow: string
  title: string
  description: string
  actions?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <section className="page-section">
      <header className="page-heading">
        <div>
          <p className="eyebrow">{eyebrow}</p>
          <h1>{title}</h1>
          <p className="page-description">{description}</p>
        </div>
        {actions}
      </header>
      {children}
    </section>
  )
}

function TimelineEvent({ event }: { event: TapeEvent }) {
  const payload = formatEventPayload(event)
  const shortened = payload.length > 2400 ? payload.slice(0, 2400) + '\n…' : payload
  return (
    <li className="event-row">
      <div className="event-rail" aria-hidden="true">
        <span />
      </div>
      <article className="event-card">
        <div className="event-card-header">
          <div>
            <span className="sequence">#{event.sequence.toString().padStart(4, '0')}</span>
            <h2>{titleCase(event.kind)}</h2>
          </div>
          <time dateTime={new Date(event.occurred_at_ms).toISOString()}>
            {formatDate(event.occurred_at_ms)}
          </time>
        </div>
        <div className="event-meta">
          <span>{titleCase(event.source)}</span>
          <span className={'redaction redaction-' + event.redaction}>
            {titleCase(event.redaction)}
          </span>
        </div>
        <details className="payload-details">
          <summary>Inspect event payload</summary>
          <pre className="payload-block">
            <code>{shortened}</code>
          </pre>
        </details>
      </article>
    </li>
  )
}
