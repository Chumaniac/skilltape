import { useEffect, useRef, useState } from 'react'
import { formatDate, formatNumber, getTapeCoverage } from '../api'
import type { TapeCoverage } from '../types'
import { PageError, PageLoading, evidenceLabel } from '../ui'

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
      if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : "无法读取采集完整性。")
    } finally {
      if (!controller.signal.aborted) setLoading(false)
    }
  }
  return <section className="panel capture-coverage" role="region" aria-labelledby="capture-coverage-title">
    <div className="panel-heading"><div><p className="eyebrow">已保存采集元数据</p><h2 id="capture-coverage-title">采集完整性</h2></div>
      <button className="button button-secondary" type="button" disabled={loading} onClick={check}>检查整份采集记录</button></div>
    <p>检查全部已记录页面的计数，最多读取100,000个事件和16 MiB内容，不执行命令。</p>
    {loading ? <PageLoading label="正在检查已保存采集事件…" /> : error ? <PageError message={error} onRetry={check} /> : data ? <div aria-live="polite">
      <p><strong>{formatNumber(data.total_events)} 条已记录事件</strong> · {data.state === 'complete' ? "已结束的采集快照" : data.state === 'open' ? "采集快照尚未结束，可能继续记录" : "检查期间采集记录发生变化，分享前请重新检查"}</p>
      <div className="metric-grid"><CoverageCount label="已脱敏" count={data.redaction.redacted} /><CoverageCount label="部分脱敏" count={data.redaction.partially_redacted} /><CoverageCount label="未脱敏" count={data.redaction.unredacted} /><CoverageCount label="采集警告" count={data.capture_warnings} /></div>
      <p className="panel-note">状态元数据不能判定内容是否适合分享，请复核原始事件和保留证据。</p>
      <div className="coverage-breakdowns"><Breakdown title="事件类型" counts={data.by_kind} /><Breakdown title="来源" counts={data.by_source} /></div>
      {data.total_events ? <p className="panel-note">首条事件： {formatDate(data.first_event_at_ms)} · 末条事件： {formatDate(data.last_event_at_ms)}</p> : <p>空采集记录没有脱敏观测数据。</p>}
      <h3>需复核事件</h3><p>显示 {data.samples.length} / {formatNumber(data.sample_total)} 个事件标记为未脱敏、部分脱敏或存在采集警告。{data.samples_truncated ? " 仅保留前32条序号引用。" : ''}</p>
      <ol className="coverage-samples">{data.samples.map(sample => <li key={sample.sequence}><a href={eventHref(sample.sequence)}>查看序号 {sample.sequence}</a><span>{evidenceLabel(sample.redaction)}{sample.capture_warning ? " · 采集警告" : ''}</span></li>)}</ol>
    </div> : <p className="panel-note">需要整体查看采集记录时再执行此检查；切换事件页不会触发扫描。</p>}
  </section>
}
function CoverageCount({ label, count }: { label: string; count: number }) { return <div className="metric"><span className="metric-label">{label}</span><strong>{formatNumber(count)}</strong></div> }
function Breakdown({ title, counts }: { title: string; counts: Record<string, number> }) { return <div><h3>{title}</h3><ul>{Object.entries(counts).map(([key,count]) => <li key={key}>{evidenceLabel(key)}: {formatNumber(count)}</li>)}</ul></div> }
