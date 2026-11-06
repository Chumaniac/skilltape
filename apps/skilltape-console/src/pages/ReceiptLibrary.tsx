import {useEffect, useRef, useState} from 'react'
import {getReceiptComparison, isSavedEvidenceId} from '../api'
import type {ReceiptComparison, ReceiptDelta} from '../receipt-comparison'
import {SavedReceipts} from './SavedReceipts'
import {VerifyAttempts} from './VerifyAttempts'

const findingLabels:Record<string,string>={package_changed:'Package changed',input_changed:'Input snapshot changed',executor_changed:'Executor changed',platform_changed:'Platform changed',limits_changed:'Resource limits changed',context_invalid:'A condition binding failed',conditions_unrecorded:'Conditions were not recorded'}
function recorded(rows:ReceiptDelta['before']) {return rows.length?rows.map(row=>row.status+(row.exit_code===null?'':` (exit ${row.exit_code})`)+` × ${row.count}`).join(' · '):'Not observed'}

export function ReceiptLibrary({onRead}:{onRead:(id:string)=>void}) {
 const [ids,setIds]=useState({before:'',after:''}),[result,setResult]=useState<ReceiptComparison|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false)
 const controller=useRef<AbortController|null>(null)
 useEffect(()=>()=>{controller.current?.abort();controller.current=null},[])
 function change(side:'before'|'after',id:string) {controller.current?.abort();controller.current=null;setBusy(false);setError('');setResult(null);setIds(previous=>({...previous,[side]:id}))}
 async function compare() {
  const pair={before:ids.before.trim().replace(/\.json$/i,''),after:ids.after.trim().replace(/\.json$/i,'')}
  if(!isSavedEvidenceId(pair.before)||!isSavedEvidenceId(pair.after)||pair.before===pair.after){setError('Choose two different saved Receipt filenames, not paths.');setResult(null);return}
  controller.current?.abort();const pending=new AbortController();controller.current=pending;setIds(pair);setResult(null);setError('');setBusy(true)
  try {const value=await getReceiptComparison(pair.before,pair.after,pending.signal);if(!pending.signal.aborted)setResult(value)}
  catch(reason){if(!pending.signal.aborted)setError(reason instanceof Error?reason.message:'Both saved Receipts could not be compared.')}
  finally{if(controller.current===pending){controller.current=null;setBusy(false)}}
 }
 return <><SavedReceipts onRead={onRead} onCompare={change}/><VerifyAttempts/><section className="panel receipt-comparison" aria-label="Compare saved Receipts">
  <h2>Compare saved outcomes</h2><p>Choose two existing filenames. The comparison reads recorded steps, checks and policy outcomes.</p>
  <form className="receipt-comparison-fields" onSubmit={event=>{event.preventDefault();void compare()}}>
   <label>Earlier Receipt ID<input value={ids.before} maxLength={255} required autoComplete="off" spellCheck={false} onChange={event=>change('before',event.target.value)}/></label>
   <label>Later Receipt ID<input value={ids.after} maxLength={255} required autoComplete="off" spellCheck={false} onChange={event=>change('after',event.target.value)}/></label>
   <button className="button" type="submit" disabled={busy}>Compare saved outcomes</button>
   {busy?<button className="button" type="button" onClick={()=>{controller.current?.abort();controller.current=null;setBusy(false);setError('Comparison cancelled. Saved Receipts are unchanged.')}}>Cancel comparison</button>:null}
  </form>
  {error?<p role="alert">{error}</p>:busy?<p role="status">Reading both saved Receipts…</p>:null}
  {result?<section aria-label="Saved Receipt comparison result" aria-live="polite">
   <h3>Recorded outcome changes</h3><p><code>{result.before.id}</code> → <code>{result.after.id}</code> · {result.before.status} → {result.after.status}</p>
   <p>{result.comparability==='incomparable'?'Conditions differ. These observations are not a regression verdict.':result.comparability==='unknown'?'Execution conditions are missing or invalid. This is a structural comparison only.':'Recorded conditions match. Inputs were not frozen and assertion criteria may differ.'}</p>
   {result.condition_findings.length?<ul>{result.condition_findings.map(code=><li key={code}>{findingLabels[code]}</li>)}</ul>:null}
   <dl className="comparison-counts"><div><dt>New non-success observations</dt><dd>{result.counts.new_non_success}</dd></div><div><dt>No longer observed</dt><dd>{result.counts.no_longer_observed_non_success}</dd></div><div><dt>Continuing non-success observations</dt><dd>{result.counts.continuing_non_success}</dd></div><div><dt>Outcome groups changed</dt><dd>{result.counts.outcome_groups_changed}</dd></div></dl>
   <p>A failure absent from the later record is not proof of a fix. Repeated checks with the same kind and target are counted together.</p>
   {result.changes.length?<div className="table-wrap" role="region" aria-label="Recorded outcome groups" tabIndex={0}><table><thead><tr><th>Group</th><th>Earlier</th><th>Later</th></tr></thead><tbody>{result.changes.map(row=><tr key={row.key}><th scope="row"><span>{row.category} · {row.kind}{row.phase?' · '+row.phase:''}</span><code>{row.target}</code>{row.label_truncated?<small>Label shortened</small>:null}</th><td>{recorded(row.before)}</td><td>{recorded(row.after)}</td></tr>)}</tbody></table></div>:<p>No recorded outcome changes to display.</p>}
   {result.changes_truncated?<p>Showing {result.changes.length} of {result.change_total} groups. Counts include every accepted row.</p>:null}
   <p className="panel-note">No replay or export approval. Receipt authorship and current files are not verified by this comparison.</p>
  </section>:null}
 </section></>
}
