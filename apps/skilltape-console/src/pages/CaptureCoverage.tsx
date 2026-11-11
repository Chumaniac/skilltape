import { useEffect, useRef, useState } from 'react'
import { formatDate, formatNumber, getTapeCoverage } from '../api'
import type { TapeCoverage } from '../types'
import { PageError, PageLoading, titleCase } from '../ui'

export function CaptureCoverage({ tapeId, eventHref }: { tapeId: string; eventHref: (sequence: number) => string }) {
  const [data, setData] = useState<TapeCoverage | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const aborter = useRef<AbortController | null>(null)
  useEffect(() => () => aborter.current?.abort(), [tapeId])
  const check = async () => {
    aborter.current?.abort()
    const controller = new AbortController(); aborter.current = controller
    setData(null); setError(''); setLoading(true)
    try {
      const value = await getTapeCoverage(tapeId, controller.signal)
      if (!controller.signal.aborted) setData(value)
    } catch (reason) {
      if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : 'Capture coverage could not be read.')
    } finally {
      if (!controller.signal.aborted) setLoading(false)
    }
  }
  return <section className="panel capture-coverage" role="region" aria-labelledby="capture-coverage-title">
    <div className="panel-heading"><div><p className="eyebrow">Stored capture metadata</p><h2 id="capture-coverage-title">Capture coverage</h2></div>
      <button className="button button-secondary" type="button" disabled={loading} onClick={check}>Check whole Tape</button></div>
    <p>Inspect counts across all recorded pages. The check reads up to 100,000 events within 16 MiB and does not run commands.</p>
    {loading ? <PageLoading label="Checking the saved capture stream…" /> : error ? <PageError message={error} onRetry={check} /> : data ? <div aria-live="polite">
      <p><strong>{formatNumber(data.total_events)} recorded events</strong> · {data.state === 'complete' ? 'Finished capture snapshot' : data.state === 'open' ? 'Open capture snapshot; recording may continue' : 'Capture changed during the check; check again before sharing'}</p>
      <div className="metric-grid"><CoverageCount label="Redacted" count={data.redaction.redacted} /><CoverageCount label="Partially redacted" count={data.redaction.partially_redacted} /><CoverageCount label="Unredacted" count={data.redaction.unredacted} /><CoverageCount label="Capture warnings" count={data.capture_warnings} /></div>
      <p className="panel-note">Status metadata cannot establish whether payload is safe to share. Review the original events and retained evidence.</p>
      <div className="coverage-breakdowns"><Breakdown title="Event kinds" counts={data.by_kind} /><Breakdown title="Sources" counts={data.by_source} /></div>
      {data.total_events ? <p className="panel-note">First recorded event: {formatDate(data.first_event_at_ms)} · Last recorded event: {formatDate(data.last_event_at_ms)}</p> : <p>No redaction observations are available for an empty capture.</p>}
      <h3>Events to review</h3><p>Showing {data.samples.length} of {formatNumber(data.sample_total)} events with an unredacted/partial label or a capture warning.{data.samples_truncated ? ' Only the first 32 sequence references are retained.' : ''}</p>
      <ol className="coverage-samples">{data.samples.map(sample => <li key={sample.sequence}><a href={eventHref(sample.sequence)}>View sequence {sample.sequence}</a><span>{titleCase(sample.redaction)}{sample.capture_warning ? ' · Capture warning' : ''}</span></li>)}</ol>
    </div> : <p className="panel-note">Run this check when you need a whole-capture overview; changing event pages does not trigger a scan.</p>}
  </section>
}
function CoverageCount({ label, count }: { label: string; count: number }) { return <div className="metric"><span className="metric-label">{label}</span><strong>{formatNumber(count)}</strong></div> }
function Breakdown({ title, counts }: { title: string; counts: Record<string, number> }) { return <div><h3>{title}</h3><ul>{Object.entries(counts).map(([key,count]) => <li key={key}>{titleCase(key)}: {formatNumber(count)}</li>)}</ul></div> }
