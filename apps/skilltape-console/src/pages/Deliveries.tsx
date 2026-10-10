import { useEffect, useState } from 'react'
import { formatNumber, getDeliveries, getDelivery } from '../api'
import type { Collection, DeliveryReview, DeliverySummary } from '../types'
import { CodeBlock, EmptyState, Metric, PageError, PageLoading, StatusBadge, titleCase } from '../ui'

interface Props { deliveryId?: string; offset: number }

function errorMessage(reason: unknown) {
  return reason instanceof Error ? reason.message : 'The saved delivery could not be inspected.'
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
    <header className="page-heading"><div><p className="eyebrow">Retained workflow output</p><h1>Review saved deliveries</h1>
      <p className="page-description">Choose an actual saved bundle, inspect its file integrity, then review its business requirements independently.</p></div>
      <button className="button button-secondary" type="button" disabled={listLoading} onClick={() => setListRevision(value => value + 1)}>Refresh delivery list</button></header>
    <div className="metric-grid">
      <Metric label="Saved directories" value={collection ? formatNumber(collection.total) : '—'} tone="accent" />
      <Metric label="This page" value={collection ? formatNumber(collection.items.length) : '—'} />
      <Metric label="Selected delivery" value={deliveryId ? '1' : '0'} />
      <Metric label="Requirements" value="Independent review" tone="warn" />
    </div>
    {listLoading ? <PageLoading label="Discovering saved delivery metadata…" /> : listError ?
      <PageError message={listError} onRetry={() => setListRevision(value => value + 1)} /> : collection && collection.total === 0 ?
      <EmptyState title="No saved deliveries found" message="Point Console at the parent of retained --delivery-dir folders. No demo or legacy run registry is substituted." /> : collection ? <article className="panel">
        <div className="panel-heading"><div><p className="eyebrow">Actual local folders</p><h2>Saved delivery queue</h2></div><span className="panel-note">Metadata only until inspected</span></div>
        <label className="delivery-filter">Filter this page<input type="search" name="deliveryFilter" autoComplete="off" spellCheck={false} maxLength={128} value={filter} onChange={event => setFilter(event.target.value)} placeholder="Folder or reported status…" /></label>
        <div className="delivery-table-wrap" tabIndex={0} aria-label="Saved delivery folders"><table className="delivery-table"><thead><tr><th>Saved folder</th><th>Receipt reports</th><th>Declared files</th><th>Review</th></tr></thead><tbody>
          {visible.map(item => <tr key={item.id} data-delivery-row><td><code>{item.id}</code></td><td><StatusBadge tone={item.metadata_valid ? 'neutral' : 'danger'}>{item.metadata_valid ? titleCase(item.reported_status ?? 'unknown') : 'Invalid metadata'}</StatusBadge></td>
            <td>{item.declared_files === null ? 'Unknown' : formatNumber(item.declared_files)}</td><td><a className="button button-secondary" aria-label={'Inspect ' + item.id} href={'#deliveries?delivery=' + encodeURIComponent(item.id) + '&deliveryOffset=' + offset}>Inspect</a></td></tr>)}
          {!visible.length ? <tr><td colSpan={4}>No match on this page. Other pages are not searched.</td></tr> : null}
        </tbody></table></div>
        <div className="delivery-pagination"><p>Page {collection.offset + 1}–{collection.offset + collection.items.length} of {collection.total} folders</p><div>
          <button className="button button-secondary" type="button" disabled={offset === 0} onClick={() => page(Math.max(0, offset - 50))}>Previous deliveries</button>
          <button className="button button-secondary" type="button" disabled={collection.next_offset === null} onClick={() => { if (collection.next_offset !== null) page(collection.next_offset) }}>Next deliveries</button>
        </div></div>
      </article> : null}
    {deliveryId ? <article className="panel delivery-selected" aria-live="polite">
      <div className="panel-heading"><div><p className="eyebrow">Selected retained bundle</p><h2><code>{deliveryId}</code></h2></div>
        <button className="button" type="button" disabled={detailLoading} onClick={() => setDetailRevision(value => value + 1)}>Recheck selected delivery</button></div>
      {detailLoading ? <PageLoading label="Checking actual files and saved evidence…" /> : detailError ? <PageError message={detailError} onRetry={() => setDetailRevision(value => value + 1)} /> : report ? <DeliveryDetails report={report} /> : null}
    </article> : <EmptyState title="Choose a saved delivery" message="Select a folder from the queue. Console does not run a workflow or manufacture a Receipt." />}
    <article className="panel delivery-next-step"><p className="eyebrow">Continue the review</p><h2>Check the declared business requirements</h2>
      <p>Intact files can still contain wrong allocations. Use a reviewed contract and the preserved delivery to create an independent SkillSync acceptance report.</p>
      <CodeBlock label="Source CLI · replace the placeholders locally" value="skillsync artifacts --contract <reviewed-contract> --delivery <saved-folder> --format html" />
      <p className="panel-note">Read-only snapshot · no commands executed · provenance is not authenticated. Preserve the successful originals.</p>
    </article>
  </section>
}

function DeliveryDetails({ report }: { report: DeliveryReview }) {
  return <>
    <div className="delivery-evidence-grid"><div><span>Reported execution</span><StatusBadge tone={report.reported_status === 'succeeded' ? 'good' : 'warn'}>{titleCase(report.reported_status ?? 'unknown')}</StatusBadge></div>
      <div><span>Physical file check</span><StatusBadge tone={report.status === 'passed' ? 'good' : 'danger'}>{report.status === 'passed' ? 'Physical files match' : 'Physical files need attention'}</StatusBadge></div>
      <div><span>Business requirements</span><StatusBadge tone="warn">Requirements not checked</StatusBadge></div></div>
    <div className="delivery-identity"><p>Receipt hash binding: <strong>{report.receipt_binding}</strong></p>
      <p>Matched files: <strong>{report.checked_files}</strong> / {report.declared_files ?? 'Unknown'} · Matched bytes: {formatNumber(report.checked_bytes)}</p>
      {report.run_id ? <details><summary>Saved run identity</summary><code>{report.run_id}</code></details> : null}</div>
    {report.findings.length ? <div className="delivery-findings" role="status"><h3>Review these saved files</h3><ul>{report.findings.map((finding, index) => <li key={index}><code>{finding.path ?? 'Bundle structure / evidence'}</code><span>{finding.code}</span></li>)}</ul>
      <p>Retain this copy for comparison. Recover known successful originals and recheck before reviewing business requirements.</p>
      {report.findings_truncated ? <p>More findings exist; this view is limited to 64.</p> : null}</div> : null}
    <div className="delivery-table-wrap" tabIndex={0} aria-label="Matched physical file metadata"><table className="delivery-table"><thead><tr><th>Matched file</th><th>Bytes</th><th>Observed SHA-256</th></tr></thead><tbody>
      {report.files.map(file => <tr key={file.path}><td><code>{file.path}</code></td><td>{formatNumber(file.bytes)}</td><td><code>{file.sha256}</code></td></tr>)}
      {!report.files.length ? <tr><td colSpan={3}>No stable matched file observation is available.</td></tr> : null}
    </tbody></table></div>
    {report.files_truncated ? <p className="panel-note">Only the first 100 matched file details are shown. The counts cover the declared scope.</p> : null}
  </>
}
