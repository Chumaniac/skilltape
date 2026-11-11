import { useEffect, useState } from 'react'
import { formatNumber, getDeliveries, getDelivery } from '../api'
import type { Collection, DeliveryReview, DeliverySummary } from '../types'
import { CodeBlock, EmptyState, Metric, PageError, PageLoading, StatusBadge, evidenceLabel } from '../ui'
import { DeliveryComparison } from './DeliveryComparison'

interface Props { deliveryId?: string; offset: number }

function errorMessage(reason: unknown) {
  return reason instanceof Error ? reason.message : "无法检查已保存交付包。"
}

export function DeliveriesPage({ deliveryId, offset }: Props) {
  const [collection, setCollection] = useState<Collection<DeliverySummary> | null>(null)
  const [report, setReport] = useState<DeliveryReview | null>(null)
  const [listError, setListError] = useState('')
  const [detailError, setDetailError] = useState('')
  const [listLoading, setListLoading] = useState(true)
  const [detailLoading, setDetailLoading] = useState(false)
  const [listRevision, setListRevision] = useState(0)
  const [detailRevision, setDetailRevision] = useState(0)
  const [filter, setFilter] = useState('')

  useEffect(() => {
    const controller = new AbortController()
    setCollection(null); setListError(''); setListLoading(true); setFilter('')
    getDeliveries(offset, controller.signal)
      .then(value => { if (!controller.signal.aborted) setCollection(value) })
      .catch(reason => { if (!controller.signal.aborted) setListError(errorMessage(reason)) })
      .finally(() => { if (!controller.signal.aborted) setListLoading(false) })
    return () => controller.abort()
  }, [offset, listRevision])

  useEffect(() => {
    const controller = new AbortController()
    setReport(null); setDetailError(''); setDetailLoading(Boolean(deliveryId))
    if (deliveryId) {
      getDelivery(deliveryId, controller.signal)
        .then(value => { if (!controller.signal.aborted) setReport(value) })
        .catch(reason => { if (!controller.signal.aborted) setDetailError(errorMessage(reason)) })
        .finally(() => { if (!controller.signal.aborted) setDetailLoading(false) })
    }
    return () => controller.abort()
  }, [deliveryId, detailRevision])

  const needle = filter.toLowerCase()
  const visible = collection?.items.filter(item => (item.id + ' ' + (item.reported_status ?? 'invalid')).toLowerCase().includes(needle)) ?? []
  const page = (next: number) => { window.location.hash = 'deliveries?deliveryOffset=' + next }
  return <section className="page-section delivery-workbench">
    <header className="page-heading"><div><p className="eyebrow">已保存输出</p><h1>复核已保存交付包</h1>
      <p className="page-description">先检查已保存文件，再复核声明的要求。</p></div>
      <button className="button button-secondary" type="button" disabled={listLoading} onClick={() => setListRevision(value => value + 1)}>刷新交付包列表</button></header>
    <div className="metric-grid">
      <Metric label="已保存目录" value={collection ? formatNumber(collection.total) : '—'} tone="accent" />
      <Metric label="当前页" value={collection ? formatNumber(collection.items.length) : '—'} />
      <Metric label="所选交付包" value={deliveryId ? '1' : '0'} />
      <Metric label="要求" value="独立复核" tone="warn" />
    </div>
    {listLoading ? <PageLoading label="正在读取交付包元数据…" /> : listError ?
      <PageError message={listError} onRetry={() => setListRevision(value => value + 1)} /> : collection && collection.total === 0 ?
      <EmptyState title="未找到已保存交付包" message="请在控制台打开已保存 --delivery-dir 交付包的父目录。" /> : collection ? <article className="panel">
        <div className="panel-heading"><div><p className="eyebrow">已保存目录</p><h2>已保存交付队列</h2></div><span className="panel-note">检查实际文件</span></div>
        <label className="delivery-filter">筛选当前页<input type="search" name="deliveryFilter" autoComplete="off" spellCheck={false} maxLength={128} value={filter} onChange={event => setFilter(event.target.value)} placeholder="目录或记录状态…" /></label>
        <div className="delivery-table-wrap" tabIndex={0} aria-label="已保存交付包目录"><table className="delivery-table"><thead><tr><th>已保存目录</th><th>凭据记录结果</th><th>声明的文件</th><th>复核</th></tr></thead><tbody>
          {visible.map(item => <tr key={item.id} data-delivery-row><td><code>{item.id}</code></td><td><StatusBadge tone={item.metadata_valid ? 'neutral' : 'danger'}>{item.metadata_valid ? evidenceLabel(item.reported_status ?? 'unknown') : "元数据无效"}</StatusBadge></td>
            <td>{item.declared_files === null ? "未知" : formatNumber(item.declared_files)}</td><td><a className="button button-secondary" aria-label={"检查 " + item.id} href={'#deliveries?delivery=' + encodeURIComponent(item.id) + '&deliveryOffset=' + offset}>检查</a></td></tr>)}
          {!visible.length ? <tr><td colSpan={4}>当前页没有匹配项，未搜索其他页面。</td></tr> : null}
        </tbody></table></div>
        <div className="delivery-pagination"><p>页 {collection.offset + 1}–{collection.offset + collection.items.length} / {collection.total} 个目录</p><div>
          <button className="button button-secondary" type="button" disabled={offset === 0} onClick={() => page(Math.max(0, offset - 50))}>上一页交付包</button>
          <button className="button button-secondary" type="button" disabled={collection.next_offset === null} onClick={() => { if (collection.next_offset !== null) page(collection.next_offset) }}>下一页交付包</button>
        </div></div>
      </article> : null}
    {collection && !listLoading && !listError && collection.items.length > 1 ? <DeliveryComparison key={offset + ':' + listRevision} items={collection.items} /> : null}
    {deliveryId ? <article className="panel delivery-selected" aria-live="polite">
      <div className="panel-heading"><div><p className="eyebrow">所选保留交付包</p><h2><code>{deliveryId}</code></h2></div>
        <button className="button" type="button" disabled={detailLoading} onClick={() => setDetailRevision(value => value + 1)}>重新检查所选交付包</button></div>
      {detailLoading ? <PageLoading label="正在检查实际文件与已保存证据…" /> : detailError ? <PageError message={detailError} onRetry={() => setDetailRevision(value => value + 1)} /> : report ? <DeliveryDetails report={report} /> : null}
    </article> : <EmptyState title="选择已保存交付包" message="从队列选择目录，检查不会执行工作流。" />}
    <article className="panel delivery-next-step"><p className="eyebrow">下一步</p><h2>核对声明的业务要求</h2>
      <p>文件完整性不代表业务要求已满足，请使用经过复核的 SkillSync 约定进行验收。</p>
      <CodeBlock label="源码 CLI · 请在本机替换占位值" value="skillsync artifacts --contract <reviewed-contract> --delivery <saved-folder> --format html" />
      <p className="panel-note">只读快照 · 未执行命令 · 未认证来源。请保留成功原件。</p>
    </article>
  </section>
}

function DeliveryDetails({ report }: { report: DeliveryReview }) {
  return <>
    <div className="delivery-evidence-grid"><div><span>记录的执行状态</span><StatusBadge tone={report.reported_status === 'succeeded' ? 'good' : 'warn'}>{evidenceLabel(report.reported_status ?? 'unknown')}</StatusBadge></div>
      <div><span>实际文件检查</span><StatusBadge tone={report.status === 'passed' ? 'good' : 'danger'}>{report.status === 'passed' ? "实际文件一致" : "实际文件需要处理"}</StatusBadge></div>
      <div><span>业务要求</span><StatusBadge tone="warn">尚未核对要求</StatusBadge></div></div>
    <div className="delivery-identity"><p>凭据哈希绑定： <strong>{report.receipt_binding}</strong></p>
      <p>匹配文件数： <strong>{report.checked_files}</strong> / {report.declared_files ?? "未知"} · 匹配字节数： {formatNumber(report.checked_bytes)}</p>
      {report.run_id ? <details><summary>已保存运行身份</summary><code>{report.run_id}</code></details> : null}</div>
    {report.findings.length ? <div className="delivery-findings" role="status"><h3>复核这些已保存文件</h3><ul>{report.findings.map((finding, index) => <li key={index}><code>{finding.path ?? "包结构与证据"}</code><span>{finding.code}</span></li>)}</ul>
      <p>请保留此副本用于对比，恢复已知成功原件并重新检查后，再复核业务要求。</p>
      {report.findings_truncated ? <p>还有更多问题，本页面最多展示64条。</p> : null}</div> : null}
    <div className="delivery-table-wrap" tabIndex={0} aria-label="实际文件元数据一致"><table className="delivery-table"><thead><tr><th>匹配文件</th><th>字节数</th><th>观测 SHA-256</th></tr></thead><tbody>
      {report.files.map(file => <tr key={file.path}><td><code>{file.path}</code></td><td>{formatNumber(file.bytes)}</td><td><code>{file.sha256}</code></td></tr>)}
      {!report.files.length ? <tr><td colSpan={3}>没有稳定且匹配的文件观测。</td></tr> : null}
    </tbody></table></div>
    {report.files_truncated ? <p className="panel-note">仅展示前100个匹配文件的明细，计数覆盖全部声明范围。</p> : null}
  </>
}
