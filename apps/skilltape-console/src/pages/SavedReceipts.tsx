import { evidenceLabel } from '../ui'
import {useEffect, useRef, useState} from 'react'
import {getReceipts, formatDate, type ReceiptFilters, type ReceiptSummary} from '../api'
import type {Collection} from '../types'

const emptyFilters: ReceiptFilters = {status: '', skillHash: ''}

export function SavedReceipts({onRead, onCompare}: {onRead: (id: string) => void; onCompare?: (side: 'before' | 'after', id: string) => void}) {
  const [data, setData] = useState<Collection<ReceiptSummary> | null>(null)
  const [draft, setDraft] = useState(emptyFilters)
  const [applied, setApplied] = useState(emptyFilters)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const aborter = useRef<AbortController | null>(null)
  useEffect(() => () => { aborter.current?.abort(); aborter.current = null }, [])

  async function load(offset = 0, filters = applied) {
    const next = {...filters, skillHash: filters.skillHash.trim().toLowerCase()}
    if (!/^[a-f0-9]{0,64}$/.test(next.skillHash)) {
      setError("指纹最多包含64位十六进制字符。")
      return
    }
    aborter.current?.abort()
    const controller = new AbortController()
    aborter.current = controller
    setBusy(true)
    setError('')
    try {
      const value = await getReceipts(controller.signal, offset, next)
      if (!controller.signal.aborted) {
        setData(value)
        setApplied(next)
        setDraft(next)
      }
    } catch (reason) {
      if (!controller.signal.aborted) {
        setError((reason instanceof Error ? reason.message : "无法列出已保存凭据。") + (data ? " 已保留此前结果与筛选条件。" : ''))
        setDraft(applied)
      }
    } finally {
      if (aborter.current === controller) { aborter.current = null; setBusy(false) }
    }
  }

  return <section className="saved-receipts" aria-label="已保存凭据库">
    <button type="button" className="button" disabled={busy} onClick={() => load(data?.offset || 0)}>{busy ? "正在读取凭据列表…" : data ? "刷新已保存凭据" : "浏览已保存凭据"}</button>
    <form onSubmit={event => {event.preventDefault(); void load(0, draft)}}>
      <fieldset className="saved-receipt-filters" disabled={busy}>
        <legend>查找已保存凭据</legend>
        <label>已记录凭据状态<select value={draft.status} onChange={event => setDraft({...draft, status: event.target.value as ReceiptFilters['status']})}>
          <option value="">全部记录状态</option><option value="succeeded">成功</option><option value="run_failed">运行失败</option><option value="assertion_failed">断言失败</option><option value="cancelled">已取消</option><option value="unavailable">元数据不可用</option>
        </select></label>
        <label>技能包指纹前缀<input value={draft.skillHash} maxLength={64} spellCheck={false} autoComplete="off" onChange={event => setDraft({...draft, skillHash: event.target.value})}/></label>
        <button type="submit" className="button">应用凭据筛选</button>
        <button type="button" className="button" onClick={() => load(0, emptyFilters)}>清除凭据筛选</button>
      </fieldset>
    </form>
    <p className="muted">搜索只读取本机元数据。不可用元数据无法匹配指纹，配合当前技能包使用前请读取完整凭据。</p>
    <p role="status">{error || (!data ? '' : data.total ? `${data.total} 份匹配凭据。记录状态不代表批准执行或导出。` : "没有符合筛选条件的已保存凭据文件。")}</p>
    {data ? <><ul className="saved-receipt-list">{data.items.map(row => <li key={row.id}>
      <div><strong>{row.id}</strong><span>{row.metadata_state === 'available' ? evidenceLabel(row.status) : "读取并检查；元数据不可用"}{row.modified_at_ms ? " · 文件已更新 " + formatDate(row.modified_at_ms) : ''}</span>{row.skill_hash ? <code>技能包指纹 {row.skill_hash.slice(0, 16)}…</code> : null}</div>
      <div className="receipt-row-actions"><button type="button" className="button" disabled={busy} aria-label={"读取凭据 " + row.id} onClick={() => onRead(row.id)}>读取</button>
        {onCompare ? <><button type="button" className="button" disabled={busy} aria-label={"使用 " + row.id + " 设为较早凭据"} onClick={() => onCompare('before', row.id)}>设为较早记录</button><button type="button" className="button" disabled={busy} aria-label={"使用 " + row.id + " 设为较新凭据"} onClick={() => onCompare('after', row.id)}>设为较新记录</button></> : null}</div>
    </li>)}</ul><nav className="page-navigation" aria-label="已保存凭据分页">
      <button className="button" disabled={busy || data.offset === 0} onClick={() => load(Math.max(0, data.offset - 50))}>上一页凭据</button>
      <button className="button" disabled={busy || data.next_offset === null} onClick={() => load(data.next_offset!)}>下一页凭据</button>
    </nav></> : null}
  </section>
}
