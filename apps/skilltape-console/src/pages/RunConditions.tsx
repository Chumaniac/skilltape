import { useEffect, useState } from 'react'
import { getRunContext, isSavedEvidenceId } from '../api'
import { compareRunContexts, type ContextReview } from '../run-context'
import { StatusBadge } from '../ui'

export function RunConditions({ receiptId }: { receiptId: string }) {
  const [review, setReview] = useState<ContextReview | null>(null)
  const [error, setError] = useState('')
  const [baseline, setBaseline] = useState('')
  const [request, setRequest] = useState<{id: string} | null>(null)
  const [comparison, setComparison] = useState<string[] | null | undefined>(undefined)
  const [comparing, setComparing] = useState(false)
  useEffect(() => {
    const controller = new AbortController()
    setReview(null); setComparing(false); setError(''); setRequest(null); setComparison(undefined); setBaseline('')
    getRunContext(receiptId, controller.signal).then(value => { if (!controller.signal.aborted) setReview(value) })
      .catch(() => { if (!controller.signal.aborted) setError("无法读取运行条件，原凭据仍可查看。") })
    return () => controller.abort()
  }, [receiptId])
  useEffect(() => {
    if (!request) return
    const controller = new AbortController()
    setComparing(true); setComparison(undefined); setError('')
    Promise.all([getRunContext(request.id, controller.signal), getRunContext(receiptId, controller.signal)])
      .then(([before, after]) => { if (!controller.signal.aborted) { setReview(after); setComparison(compareRunContexts(before, after)) } })
      .catch(() => { if (!controller.signal.aborted) setError("无法比较所选运行条件，请检查两份已保存 ID。") })
      .finally(() => { if (!controller.signal.aborted) setComparing(false) })
    return () => controller.abort()
  }, [request, receiptId])
  const context = review?.context
  return <section className="panel" aria-label="运行条件">
    <h2>运行条件</h2>
    {error ? <p role="alert">{error}</p> : !review ? <p role="status">正在读取已保存运行条件…</p> : review.status === 'unrecorded' ? <p>此凭据未记录。</p> : review.status === 'invalid' ? <p role="alert">运行条件绑定失败，暂时无法读取条件。</p> : context ? <>
      <StatusBadge tone="neutral">已记录 · 凭据字节一致</StatusBadge>
      <dl className="receipt-facts">
        <div><dt>输入快照</dt><dd className="context-digest"><code>{context.input_set_sha256}</code></dd></div>
        <div><dt>技能包</dt><dd className="context-digest"><code>{context.skill_hash}</code></dd></div>
        <div><dt>执行器</dt><dd>{context.executor_version} · {context.os}/{context.arch}</dd></div>
        <div><dt>已配置上限</dt><dd>{context.max_processes} 个进程 · {context.step_timeout_ms} 毫秒/步 · {context.max_output_bytes} 输出字节数</dd></div>
      </dl>
    </> : null}
    <form className="receipt-picker-controls" onSubmit={event => {
      event.preventDefault()
      const id = baseline.trim().replace(/\.json$/i, '')
      if (!isSavedEvidenceId(id) || id === receiptId) { setError("请选择另一个已保存凭据文件名。"); return }
      setRequest({id})
    }}>
      <label htmlFor="context-baseline">基准凭据 ID</label>
      <input id="context-baseline" value={baseline} maxLength={255} autoComplete="off" spellCheck={false} required onChange={event => { setBaseline(event.target.value); setError(''); setRequest(null); setComparison(undefined); setComparing(false) }} />
      <button className="button" type="submit" disabled={comparing || !baseline || !review}>对比运行条件</button>
    </form>
    {comparing ? <p role="status">正在读取两份已保存记录…</p> : comparison === null ? <p>运行条件缺失或绑定失败，暂时无法对比。</p> : comparison ? comparison.length ? <ul aria-label="观测到的运行条件差异">{comparison.map(item => <li key={item}>{item}</li>)}</ul> : <p>未记录条件差异。</p> : null}
    <p className="panel-note">摘要来自运行前的有界输入扫描，仅作为本机元数据；不保证输入已冻结、作者身份真实或能够自动重放。</p>
  </section>
}
