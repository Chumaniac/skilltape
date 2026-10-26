import { useEffect, useState } from 'react'
import { ApiError, getBatchReview } from '../api'
import type { BatchCase, BatchReview, BatchStatus } from '../batch'
import { CodeBlock, EmptyState, Metric, PageError, PageLoading, StatusBadge, titleCase } from '../ui'

const tone=(status:BatchStatus)=>status==='succeeded'?'good':status==='failed'||status==='unconfirmed'?'danger':status==='cancelled'?'warn':'neutral'
const receiptHref=(id:string)=>'#receipt?run='+encodeURIComponent(id)
const deliveryHref=(id:string)=>'#deliveries?delivery='+encodeURIComponent(id)

export function BatchReviewPage(){
 const [report,setReport]=useState<BatchReview|null>(null),[loading,setLoading]=useState(true),[missing,setMissing]=useState(false),[error,setError]=useState(''),[revision,setRevision]=useState(0),[attention,setAttention]=useState(false)
 useEffect(()=>{
  const controller=new AbortController();setReport(null);setError('');setMissing(false);setLoading(true)
  getBatchReview(controller.signal).then(value=>{if(!controller.signal.aborted)setReport(value)}).catch(reason=>{
   if(controller.signal.aborted)return
   if(reason instanceof ApiError&&reason.status===404)setMissing(true)
   else setError(reason instanceof Error?reason.message:'Batch progress could not be read.')
  }).finally(()=>{if(!controller.signal.aborted)setLoading(false)})
  return ()=>controller.abort()
 },[revision])
 const visible=report?.cases.filter(row=>!attention||row.status!=='succeeded')??[]
 return <section className="page-section delivery-workbench batch-workbench">
  <header className="page-heading"><div><p className="eyebrow">Multiple frozen input sets</p><h1>Review a verification batch</h1><p className="page-description">Review saved progress, preserve successful attempts, and inspect cases needing attention.</p></div><button className="button button-secondary" disabled={loading} onClick={()=>setRevision(value=>value+1)}>Refresh batch</button></header>
  {loading?<PageLoading label="Reading batch progress…"/>:error?<PageError message={error} onRetry={()=>setRevision(value=>value+1)}/>:missing?<EmptyState title="No batch workspace found" message="Prepare a batch with the source CLI, then open Console with that batch workspace."/>:report?<>
   <div className="metric-grid"><Metric label="Cases" value={String(report.cases.length)} tone="accent"/><Metric label="Completed" value={String(report.cases.filter(row=>row.status==='succeeded').length)} tone="good"/><Metric label="Need attention" value={String(report.cases.filter(row=>row.status!=='succeeded').length)} tone="warn"/><Metric label="Recorded revision" value={String(report.revision)}/></div>
   <article className="panel"><div className="panel-heading"><div><p className="eyebrow">Saved case queue</p><h2>Reported batch: {titleCase(report.status)}</h2></div><span className="panel-note">Read-only · explicit CLI execution</span></div>
    <label className="batch-attention-filter"><input type="checkbox" checked={attention} onChange={event=>setAttention(event.target.checked)}/> Only cases needing attention</label>
    <div className="delivery-table-wrap" tabIndex={0} aria-label="Batch case progress"><table className="delivery-table"><thead><tr><th>Case</th><th>Reported outcome</th><th>Saved evidence</th></tr></thead><tbody>{visible.map(row=><BatchRow key={row.id} row={row}/>)}{!visible.length?<tr><td colSpan={3}>No cases need attention in this recorded batch.</td></tr>:null}</tbody></table></div>
   </article>
   <article className="panel delivery-next-step"><h2>Continue the recorded workflow</h2><p>Run only pending cases from the CLI. A failed or cancelled case requires an explicit retry; successful attempts are retained. Unknown outcomes are inspected and reconciled before any further execution.</p>
    <CodeBlock label="Source CLI · replace local placeholders" value={'skilltape batch run <batch-workspace> --yes\nskilltape batch retry <batch-workspace> --case <failed-case> --input <corrected-input> --yes\nskilltape batch reconcile <batch-workspace>'}/>
    <details><summary>Recorded material bindings</summary><p>Skill: <code>{report.package_sha256}</code></p><p>Plan: <code>{report.plan_sha256}</code></p></details>
    <p className="panel-note">Receipt bindings are checked here. Open a delivery to inspect physical files. Local hashes do not authenticate authors or establish business acceptance. Limits: {report.max_cases} cases, {report.max_attempts} attempts per case.</p>
   </article>
  </>:null}
 </section>
}

function BatchRow({row}:{row:BatchCase}){
 return <tr data-batch-case><td><code>{row.id}</code><p>{row.attempt_count} recorded attempt{row.attempt_count===1?'':'s'}</p></td><td><StatusBadge tone={tone(row.status)}>{titleCase(row.status)}</StatusBadge>{row.evidence==='changed'?<p>Saved evidence changed or is missing. Inspect before continuing.</p>:row.status==='unconfirmed'?<p>Outcome is not confirmed. It will not retry automatically.</p>:null}</td><td>
  {row.receipt_id?<a className="button button-secondary" aria-label={'Open Receipt for '+row.id} href={receiptHref(row.receipt_id)}>Open Receipt</a>:<span>No completed Receipt recorded</span>}
  {row.delivery_id?<a className="button button-secondary" aria-label={'Inspect delivery for '+row.id} href={deliveryHref(row.delivery_id)}>Inspect delivery</a>:null}
  {row.attempts.length>1?<details><summary>Earlier attempts</summary>{row.attempts.slice(0,-1).map(attempt=><p key={attempt.number}>Attempt {attempt.number}: {titleCase(attempt.status)} · {attempt.receipt_id?<a aria-label={`Receipt for ${row.id} attempt ${attempt.number}`} href={receiptHref(attempt.receipt_id)}>Receipt</a>:'No completed evidence'}</p>)}</details>:null}
 </td></tr>
}
