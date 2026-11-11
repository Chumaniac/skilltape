import { useEffect, useState } from 'react'

import { formatDate, formatEventPayload, formatNumber, getTapeEvents, getTapes, getWorkspaces } from '../api'
import type { Collection, TapeEvent, TapeEvents, TapeSummary, WorkspaceSummary } from '../types'
import { TapePackages } from './TapePackages'
import { CaptureCoverage } from './CaptureCoverage'
import { EmptyState, PageError, PageLoading, evidenceLabel } from '../ui'

interface TimelineProps {
  tapeId?: string
  tapeOffset?: number
  eventOffset?: number
}

interface TimelineData {
  workspace: WorkspaceSummary | null
  tapes: Collection<TapeSummary>
  events: TapeEvents | null
}

export function TimelinePage({ tapeId, tapeOffset = 0, eventOffset = 0 }: TimelineProps) {
  const [data, setData] = useState<TimelineData | null>(null)
  const [error, setError] = useState("无法载入本机采集时间线。")
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setError('')
    Promise.all([getWorkspaces(controller.signal), getTapes('default', controller.signal, tapeOffset)])
      .then(async ([workspaces, tapes]) => {
        const selectedTape = tapeId ?? tapes.items[0]?.id
        const events = selectedTape
          ? await getTapeEvents(selectedTape, controller.signal, eventOffset)
          : null
        if (controller.signal.aborted) return
        setData({
          workspace: workspaces.items[0] ?? null,
          tapes,
          events,
        })
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : "无法读取本机 API。")
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [reload, tapeId, tapeOffset, eventOffset])

  if (loading) return <PageLoading label="正在载入采集时间线…" />
  if (error) return <PageError message={error} onRetry={() => setReload((value) => value + 1)} />
  if (!data || data.tapes.total === 0) {
    return (
      <PageSection
        eyebrow="已保存采集记录"
        title="采集时间线"
        description="查看已记录的终端、文件和权限事件。"
      >
        <EmptyState
          title="暂无采集记录"
          message="运行 skilltape capture，在此工作区创建首份脱敏采集记录。"
        />
      </PageSection>
    )
  }

  const selectedTape = data.events?.tape_id ?? data.tapes.items[0]?.id
  const selectedSummary = data.tapes.items.find((tape) => tape.id === selectedTape)
  const tapeLink = (nextTape: string | undefined, nextTapeOffset: number, nextEventOffset: number) => {
    const params = new URLSearchParams()
    if (nextTape) params.set('tape', nextTape)
    if (nextTapeOffset) params.set('tapesOffset', String(nextTapeOffset))
    if (nextEventOffset) params.set('eventsOffset', String(nextEventOffset))
    return '#timeline?' + params.toString()
  }
  const navigate = (nextTape: string | undefined, nextTapeOffset: number, nextEventOffset: number) => {
    window.location.hash = tapeLink(nextTape,nextTapeOffset,nextEventOffset)
  }
  return (
    <PageSection
      eyebrow="已保存采集记录"
      title="采集时间线"
      description="每个事件均展示来源与脱敏状态。"
      actions={
        <label className="select-control">
          <span>采集记录</span>
          <select
            aria-label="选择采集记录"
            name="tape"
            value={selectedTape}
            onChange={(event) => {
              navigate(event.target.value, tapeOffset, 0)
            }}
          >
            {selectedTape && !selectedSummary ? <option value={selectedTape}>{selectedTape}</option> : null}
            {data.tapes.items.map((tape) => (
              <option key={tape.id} value={tape.id}>
                {tape.id}
              </option>
            ))}
          </select>
        </label>
      }
    >
      <PageNavigation
        label="采集记录"
        offset={data.tapes.offset}
        count={data.tapes.items.length}
        limit={data.tapes.limit}
        total={data.tapes.total}
        nextOffset={data.tapes.next_offset}
        onPage={(offset) => navigate(undefined, offset, 0)}
      />
      <div className="metric-grid">
        <div className="metric metric-accent">
          <span className="metric-label">工作区</span>
          <strong>{data.workspace?.name ?? "本地工作区"}</strong>
        </div>
        <div className="metric">
          <span className="metric-label">事件</span>
          <strong>{formatNumber(data.events?.total ?? 0)}</strong>
        </div>
        <div className="metric">
          <span className="metric-label">采集状态</span>
          <strong>{!selectedSummary ? "未知" : selectedSummary.finished_at_ms !== null ? "已结束" : "尚未结束"}</strong>
        </div>
      </div>
      {selectedTape ? <TapePackages key={selectedTape} tapeId={selectedTape} /> : null}
      {selectedTape ? <CaptureCoverage key={selectedTape} tapeId={selectedTape} eventHref={sequence => tapeLink(selectedTape,tapeOffset,Math.floor(sequence/100)*100)} /> : null}
      {data.events ? (
        <PageNavigation
          label="事件"
          offset={data.events.offset}
          count={data.events.events.length}
          limit={data.events.limit}
          total={data.events.total}
          nextOffset={data.events.next_offset}
          onPage={(offset) => navigate(selectedTape, tapeOffset, offset)}
        />
      ) : null}
      {data.events && data.events.events.length > 0 ? (
        <ol className="event-list" aria-label="采集事件">
          {data.events.events.map((event) => (
            <TimelineEvent event={event} key={event.sequence} />
          ))}
        </ol>
      ) : (
        <EmptyState
          title={data.events?.total ? "当前页没有事件" : "采集记录为空"}
          message={data.events?.total ? "使用“上一页事件”返回之前页面。" : "此采集记录没有保存事件。"}
        />
      )}
    </PageSection>
  )
}

function PageNavigation({
  label, offset, count, limit, total, nextOffset, onPage,
}: {
  label: string
  offset: number
  count: number
  limit: number
  total: number
  nextOffset: number | null
  onPage: (offset: number) => void
}) {
  if (total <= limit && offset === 0) return null
  return (
    <nav className="page-navigation" aria-label={label + '分页'}>
      <button className="button button-secondary" disabled={offset === 0} onClick={() => onPage(Math.max(0, offset - limit))}>
        上一页{label}
      </button>
      <span>{label} {count === 0 ? 0 : formatNumber(offset + 1)}–{formatNumber(offset + count)} / {formatNumber(total)}</span>
      <button className="button button-secondary" disabled={nextOffset === null} onClick={() => { if (nextOffset !== null) onPage(nextOffset) }}>
        下一页{label}
      </button>
    </nav>
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
            <h2>{evidenceLabel(event.kind)}</h2>
          </div>
          <time dateTime={new Date(event.occurred_at_ms).toISOString()}>
            {formatDate(event.occurred_at_ms)}
          </time>
        </div>
        <div className="event-meta">
          <span>{evidenceLabel(event.source)}</span>
          <span className={'redaction redaction-' + event.redaction}>
            {evidenceLabel(event.redaction)}
          </span>
        </div>
        <details className="payload-details">
          <summary>查看事件内容</summary>
          <pre className="payload-block">
            <code>{shortened}</code>
          </pre>
        </details>
      </article>
    </li>
  )
}
