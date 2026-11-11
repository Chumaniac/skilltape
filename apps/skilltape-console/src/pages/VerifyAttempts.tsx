import {useEffect,useRef,useState} from 'react'
import {getVerifyAttempts,getVerifyAttempt,formatDate} from '../api'
import type {Collection} from '../types'
import type {VerifyAttempt} from '../verify-attempts'

const labels={receipt_produced:'Receipt produced',preparation_failed:'Preparation failed',unconfirmed:'Completion unconfirmed'}
export function VerifyAttempts(){
 const [data,setData]=useState<Collection<VerifyAttempt>|null>(null),[selected,setSelected]=useState<VerifyAttempt|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('')
 const controller=useRef<AbortController|null>(null)
 useEffect(()=>()=>{controller.current?.abort();controller.current=null},[])
 async function read(id?:string,offset=0){
  controller.current?.abort();const pending=new AbortController();controller.current=pending;setBusy(true);setError('');setSelected(null);if(!id)setData(null)
  try{if(id){const value=await getVerifyAttempt(id,pending.signal);if(controller.current===pending&&!pending.signal.aborted)setSelected(value)}
   else{const value=await getVerifyAttempts(pending.signal,offset);if(controller.current===pending&&!pending.signal.aborted)setData(value)}}
  catch(reason){if(controller.current===pending&&!pending.signal.aborted)setError(reason instanceof Error?reason.message:'Attempt records could not be read.')}
  finally{if(controller.current===pending){controller.current=null;setBusy(false)}}
 }
 return <section className="panel verify-attempts" aria-label="Single verification attempts">
  <h2>Single verification attempts</h2><p>Read saved start and completion records. This page does not run or retry a command.</p>
  <p>Record a future single verification with <code>--attempt-dir WORKSPACE/.skilltape/attempts</code>, outside its Skill and input directories.</p>
  <button className="button" type="button" disabled={busy} onClick={()=>void read()}>Browse single attempts</button>
  {busy?<button className="button" type="button" onClick={()=>{controller.current?.abort();controller.current=null;setBusy(false);setSelected(null);setError('Read cancelled. Saved records are unchanged.')}}>Cancel attempt read</button>:null}
  {error?<p role="alert">{error}</p>:busy?<p role="status">Reading saved attempt metadata…</p>:null}
  {data?<><p>{data.total} saved attempts · up to 256 in this directory</p>{data.items.length?<ul className="attempt-list">{data.items.map(row=><li key={row.id}><div><strong>{labels[row.outcome]}</strong><small>{formatDate(row.started_at_ms)}</small><code>{row.id}</code></div><button className="button" type="button" disabled={busy} aria-label={'Read attempt '+row.id} onClick={()=>void read(row.id)}>Read attempt</button></li>)}</ul>:<p>No saved single verification attempts in this workspace.</p>}
   <div className="actions"><button className="button" type="button" disabled={busy||data.offset===0} onClick={()=>void read(undefined,Math.max(0,data.offset-12))}>Previous attempts</button><button className="button" type="button" disabled={busy||data.next_offset===null} onClick={()=>void read(undefined,data.next_offset??0)}>Next attempts</button></div></>:null}
  {selected?<section aria-label="Single attempt record" aria-live="polite"><h3>{labels[selected.outcome]}</h3><code className="attempt-id">{selected.id}</code>
   <p>{selected.outcome==='preparation_failed'?'No verification was submitted to the runner. Check the setup before explicitly starting another command.':selected.outcome==='unconfirmed'?'There is no confirmed completion. The original command may still be active or may have been interrupted. Check its process and retained output before starting another verification.':'A Receipt was generated with the recorded status below. Its saved file has not been verified by this metadata read.'}</p>
   {selected.receipt?<dl><dt>Recorded Receipt status</dt><dd>{selected.receipt.status}</dd><dt>Recorded Receipt SHA-256</dt><dd><code className="attempt-id">{selected.receipt.receipt_sha256}</code></dd><dt>Recorded Skill fingerprint</dt><dd><code className="attempt-id">{selected.receipt.skill_hash}</code></dd></dl>:null}
   <p className="panel-note">{selected.record_integrity==='start-only'?'Only a start record is available.':'Start and completion metadata match.'} Local records do not authenticate their author or grant replay/export approval.</p>
  </section>:null}
 </section>
}
