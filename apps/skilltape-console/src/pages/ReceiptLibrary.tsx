import { evidenceLabel } from '../ui'
import {useEffect, useRef, useState} from 'react'
import {getReceiptComparison, isSavedEvidenceId} from '../api'
import type {ReceiptComparison, ReceiptDelta} from '../receipt-comparison'
import {SavedReceipts} from './SavedReceipts'
import {VerifyAttempts} from './VerifyAttempts'

const findingLabels:Record<string,string>={package_changed:"技能包发生变化",input_changed:"输入快照发生变化",executor_changed:"执行器发生变化",platform_changed:"运行平台发生变化",limits_changed:"资源上限发生变化",context_invalid:"运行条件绑定失败",conditions_unrecorded:"未记录运行条件"}
function recorded(rows:ReceiptDelta['before']) {return rows.length?rows.map(row=>evidenceLabel(row.status)+(row.exit_code===null?'':`（退出码 ${row.exit_code}）`)+` × ${row.count}`).join(' · '):"尚未观测"}

export function ReceiptLibrary({onRead}:{onRead:(id:string)=>void}) {
 const [ids,setIds]=useState({before:'',after:''}),[result,setResult]=useState<ReceiptComparison|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false)
 const controller=useRef<AbortController|null>(null)
 useEffect(()=>()=>{controller.current?.abort();controller.current=null},[])
 function change(side:'before'|'after',id:string) {controller.current?.abort();controller.current=null;setBusy(false);setError('');setResult(null);setIds(previous=>({...previous,[side]:id}))}
 async function compare() {
  const pair={before:ids.before.trim().replace(/\.json$/i,''),after:ids.after.trim().replace(/\.json$/i,'')}
  if(!isSavedEvidenceId(pair.before)||!isSavedEvidenceId(pair.after)||pair.before===pair.after){setError("请选择两个不同的已保存凭据文件名，不要填写路径。");setResult(null);return}
  controller.current?.abort();const pending=new AbortController();controller.current=pending;setIds(pair);setResult(null);setError('');setBusy(true)
  try {const value=await getReceiptComparison(pair.before,pair.after,pending.signal);if(!pending.signal.aborted)setResult(value)}
  catch(reason){if(!pending.signal.aborted)setError(reason instanceof Error?reason.message:"无法比较这两份已保存凭据。")}
  finally{if(controller.current===pending){controller.current=null;setBusy(false)}}
 }
 return <><SavedReceipts onRead={onRead} onCompare={change}/><VerifyAttempts/><section className="panel receipt-comparison" aria-label="比较已保存凭据">
  <h2>比较已保存结果</h2><p>请选择两个已有文件名，对比将读取记录中的步骤、检查项和策略结果。</p>
  <form className="receipt-comparison-fields" onSubmit={event=>{event.preventDefault();void compare()}}>
   <label>较早凭据 ID<input value={ids.before} maxLength={255} required autoComplete="off" spellCheck={false} onChange={event=>change('before',event.target.value)}/></label>
   <label>较新凭据 ID<input value={ids.after} maxLength={255} required autoComplete="off" spellCheck={false} onChange={event=>change('after',event.target.value)}/></label>
   <button className="button" type="submit" disabled={busy}>比较已保存结果</button>
   {busy?<button className="button" type="button" onClick={()=>{controller.current?.abort();controller.current=null;setBusy(false);setError("已取消对比，已保存凭据保持不变。")}}>取消对比</button>:null}
  </form>
  {error?<p role="alert">{error}</p>:busy?<p role="status">正在读取两份已保存凭据…</p>:null}
  {result?<section aria-label="已保存凭据对比结果" aria-live="polite">
   <h3>记录中的结果变化</h3><p><code>{result.before.id}</code> → <code>{result.after.id}</code> · {evidenceLabel(result.before.status)} → {evidenceLabel(result.after.status)}</p>
   <p>{result.comparability==='incomparable'?"运行条件存在差异，这些观测不能直接判定为回归。":result.comparability==='unknown'?"运行条件缺失或无效，仅可比较结构。":"已记录条件一致。输入未冻结，断言条件也可能不同。"}</p>
   {result.condition_findings.length?<ul>{result.condition_findings.map(code=><li key={code}>{findingLabels[code]}</li>)}</ul>:null}
   <dl className="comparison-counts"><div><dt>新增非成功结果</dt><dd>{result.counts.new_non_success}</dd></div><div><dt>不再出现</dt><dd>{result.counts.no_longer_observed_non_success}</dd></div><div><dt>持续出现的非成功结果</dt><dd>{result.counts.continuing_non_success}</dd></div><div><dt>结果分组发生变化</dt><dd>{result.counts.outcome_groups_changed}</dd></div></dl>
   <p>后一次记录未出现某项失败，并不能证明问题已修复。同类型、同目标的重复检查会合并计数。</p>
   {result.changes.length?<div className="table-wrap" role="region" aria-label="记录中的结果分组" tabIndex={0}><table><thead><tr><th>分组</th><th>较早记录</th><th>较新记录</th></tr></thead><tbody>{result.changes.map(row=><tr key={row.key}><th scope="row"><span>{evidenceLabel(row.category)} · {evidenceLabel(row.kind)}{row.phase?' · '+evidenceLabel(row.phase):''}</span><code>{row.target}</code>{row.label_truncated?<small>标签已缩短</small>:null}</th><td>{recorded(row.before)}</td><td>{recorded(row.after)}</td></tr>)}</tbody></table></div>:<p>没有可展示的结果变化。</p>}
   {result.changes_truncated?<p>显示 {result.changes.length} / {result.change_total} 个分组，计数包含全部有效记录。</p>:null}
   <p className="panel-note">本次对比不授予重放或导出许可，也未验证凭据作者和当前文件。</p>
  </section>:null}
 </section></>
}
