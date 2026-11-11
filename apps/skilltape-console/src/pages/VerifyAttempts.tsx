import { evidenceLabel } from '../ui'
import {useEffect,useRef,useState} from 'react'
import {getVerifyAttempts,getVerifyAttempt,formatDate} from '../api'
import type {Collection} from '../types'
import type {VerifyAttempt} from '../verify-attempts'

const labels={receipt_produced:"已生成凭据",preparation_failed:"准备失败",unconfirmed:"完成状态未确认"}
export function VerifyAttempts(){
 const [data,setData]=useState<Collection<VerifyAttempt>|null>(null),[selected,setSelected]=useState<VerifyAttempt|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('')
 const controller=useRef<AbortController|null>(null)
 useEffect(()=>()=>{controller.current?.abort();controller.current=null},[])
 async function read(id?:string,offset=0){
  controller.current?.abort();const pending=new AbortController();controller.current=pending;setBusy(true);setError('');setSelected(null);if(!id)setData(null)
  try{if(id){const value=await getVerifyAttempt(id,pending.signal);if(controller.current===pending&&!pending.signal.aborted)setSelected(value)}
   else{const value=await getVerifyAttempts(pending.signal,offset);if(controller.current===pending&&!pending.signal.aborted)setData(value)}}
  catch(reason){if(controller.current===pending&&!pending.signal.aborted)setError(reason instanceof Error?reason.message:"无法读取尝试记录。")}
  finally{if(controller.current===pending){controller.current=null;setBusy(false)}}
 }
 return <section className="panel verify-attempts" aria-label="单次验证尝试">
  <h2>单次验证尝试</h2><p>读取已保存的开始和完成记录。本页面不会执行或重试命令。</p>
  <p>记录下一次单独验证时使用 <code>--attempt-dir WORKSPACE/.skilltape/attempts</code>，并放在技能包与输入目录之外。</p>
  <button className="button" type="button" disabled={busy} onClick={()=>void read()}>浏览单次尝试</button>
  {busy?<button className="button" type="button" onClick={()=>{controller.current?.abort();controller.current=null;setBusy(false);setSelected(null);setError("已取消读取，已保存记录保持不变。")}}>取消读取尝试</button>:null}
  {error?<p role="alert">{error}</p>:busy?<p role="status">正在读取尝试元数据…</p>:null}
  {data?<><p>{data.total} 次已保存尝试 · 此目录最多256条</p>{data.items.length?<ul className="attempt-list">{data.items.map(row=><li key={row.id}><div><strong>{labels[row.outcome]}</strong><small>{formatDate(row.started_at_ms)}</small><code>{row.id}</code></div><button className="button" type="button" disabled={busy} aria-label={"读取尝试 "+row.id} onClick={()=>void read(row.id)}>读取尝试</button></li>)}</ul>:<p>此工作区没有保存单次验证尝试。</p>}
   <div className="actions"><button className="button" type="button" disabled={busy||data.offset===0} onClick={()=>void read(undefined,Math.max(0,data.offset-12))}>上一页尝试</button><button className="button" type="button" disabled={busy||data.next_offset===null} onClick={()=>void read(undefined,data.next_offset??0)}>下一页尝试</button></div></>:null}
  {selected?<section aria-label="单次尝试记录" aria-live="polite"><h3>{labels[selected.outcome]}</h3><code className="attempt-id">{selected.id}</code>
   <p>{selected.outcome==='preparation_failed'?"未向执行器提交验证，请先检查配置，再明确启动下一条命令。":selected.outcome==='unconfirmed'?"尚无确认的完成记录。原命令可能仍在运行或曾被中断，请先核对进程及保留输出，再启动新的验证。":"已生成验证凭据，记录状态如下。本次读取仅检查元数据，尚未验证其文件内容。"}</p>
   {selected.receipt?<dl><dt>已记录凭据状态</dt><dd>{evidenceLabel(selected.receipt.status)}</dd><dt>已记录凭据 SHA-256</dt><dd><code className="attempt-id">{selected.receipt.receipt_sha256}</code></dd><dt>已记录技能包指纹</dt><dd><code className="attempt-id">{selected.receipt.skill_hash}</code></dd></dl>:null}
   <p className="panel-note">{selected.record_integrity==='start-only'?"仅有开始记录。":"开始与完成元数据一致。"} 本机记录不认证作者身份，也不授予重放或导出许可。</p>
  </section>:null}
 </section>
}
