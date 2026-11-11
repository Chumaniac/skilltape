import { useEffect, useState } from 'react'
import { formatNumber, getDelivery } from '../api'
import { compareDeliveries } from '../delivery-comparison'
import type { DeliverySummary } from '../types'
import { PageError, PageLoading, StatusBadge, evidenceLabel } from '../ui'

export function DeliveryComparison({ items }: { items: DeliverySummary[] }) {
  const [before, setBefore] = useState('')
  const [after, setAfter] = useState('')
  const [request, setRequest] = useState<{ before: string; after: string } | null>(null)
  const [result, setResult] = useState<ReturnType<typeof compareDeliveries>>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)
  const [inspected, setInspected] = useState(false)

  useEffect(() => {
    const controller = new AbortController()
    setResult(null); setError(''); setInspected(false); setLoading(Boolean(request))
    if (request) {
      Promise.all([getDelivery(request.before, controller.signal), getDelivery(request.after, controller.signal)])
        .then(([a, b]) => { if (!controller.signal.aborted) { setResult(compareDeliveries(a, b)); setInspected(true) } })
        .catch(reason => {
          if (!controller.signal.aborted) {
            setError(reason instanceof Error ? reason.message : "无法比较这两个交付包。")
            setLoading(false); controller.abort()
          }
        })
        .finally(() => { if (!controller.signal.aborted) setLoading(false) })
    }
    return () => controller.abort()
  }, [request])

  const choose = (value: string, baseline: boolean) => {
    setRequest(null); setResult(null); setInspected(false); setError('')
    if (baseline) setBefore(value); else setAfter(value)
  }
  return <article className="panel delivery-comparison" aria-live="polite">
    <div className="panel-heading"><div><h2>比较已保存交付包</h2></div></div>
    <p>选择两个已保存交付包比较文件变化；两个包都会重新检查。</p>
    <div className="delivery-comparison-controls">
      <label>基准交付包<select value={before} onChange={event => choose(event.target.value, true)}>
        <option value="">选择基准版本…</option>{items.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}
      </select></label>
      <label>替换交付包<select value={after} onChange={event => choose(event.target.value, false)}>
        <option value="">选择替换版本…</option>{items.map(item => <option key={item.id} value={item.id}>{item.id}</option>)}
      </select></label>
      <button className="button" type="button" disabled={loading || !before || !after || before === after}
        onClick={() => setRequest({ before, after })}>比较文件变化</button>
    </div>
    {loading ? <PageLoading label="正在重新检查两个交付包…" /> : error ? <PageError message={error} onRetry={() => setRequest({ before, after })} /> : inspected ? result ? <>
      <StatusBadge tone="neutral">文件对比完成</StatusBadge>
      <p>{result.counts.added} 新增 · {result.counts.removed} 移除 · {result.counts.changed} 变化 · {result.counts.unchanged} 未变化</p>
      <div className="delivery-table-wrap" tabIndex={0} aria-label="已保存交付包的文件变化"><table className="delivery-table">
        <thead><tr><th>文件</th><th>变化</th><th>基准字节数</th><th>替换版本字节数</th><th>基准 SHA-256</th><th>替换版本 SHA-256</th></tr></thead>
        <tbody>{result.changes.map(row => <tr key={row.path}><td><code>{row.path}</code></td><td>{evidenceLabel(row.kind)}</td>
          <td>{row.before ? formatNumber(row.before.bytes) : "缺失"}</td><td>{row.after ? formatNumber(row.after.bytes) : "缺失"}</td>
          <td><code>{row.before?.sha256 ?? "缺失"}</code></td><td><code>{row.after?.sha256 ?? "缺失"}</code></td></tr>)}
          {!result.changes.length ? <tr><td colSpan={6}>未发现文件内容或大小变化。</td></tr> : null}</tbody>
      </table></div>
      {result.truncated ? <p>计数涵盖两个交付包，仅展示前100条发生变化的记录。</p> : null}
    </> : <><StatusBadge tone="warn">暂时无法对比</StatusBadge><p>实际文件证据不完整或检查失败，请重新检查原交付包后再对比。</p></> : null}
    <details className="panel-note"><summary>对比范围</summary><p>两个交付包均须完成一致性检查，每包最多100个文件。本机快照对比不会执行内容或认证作者身份；业务要求需要单独复核。</p></details>
  </article>
}
