import {useEffect,useRef,useState} from 'react'
import {getTapePackages,type TapePackages as Discovery} from '../api'
export function TapePackages({tapeId}:{tapeId:string}){
 const [data,setData]=useState<Discovery|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false)
 const request=useRef<AbortController|null>(null)
 useEffect(()=>()=>request.current?.abort(),[tapeId])
 async function load(offset=0){request.current?.abort();const controller=new AbortController();request.current=controller;setBusy(true);setError('');try{const value=await getTapePackages(tapeId,controller.signal,offset);if(!controller.signal.aborted)setData(value)}catch(reason){if(!controller.signal.aborted){setData(null);setError(reason instanceof Error?reason.message:"技能包查询失败，请重试或输入已保存技能包 ID。")}}finally{if(request.current===controller){request.current=null;setBusy(false)}}}
 const matches=data?.items.filter(row=>row.matches_tape)||[]
 return <section className="panel" aria-labelledby="capture-packages-title"><h2 id="capture-packages-title">从本次采集继续</h2><p>查找清单与事件摘要均匹配这份完整采集记录的已保存技能包。查询只读取本机元数据，不编译或执行内容。</p><button type="button" className="button" disabled={busy} onClick={()=>load(data?.offset||0)}>{busy?"正在检查已保存技能包…":data?"刷新技能包查询":"查找已编译技能包"}</button><div role="status" aria-live="polite">{error||(!data?'':matches.length?`${matches.length} 个匹配技能包 在当前页。`:"当前页没有匹配技能包。可查看下一页、用 CLI 编译此采集记录，或打开已知技能包 ID。")}</div>
 {matches.length?<nav className="journey-actions" aria-label="由本次采集编译的技能包">{matches.map(row=><a className="button" key={row.id} href={'#compile?skill='+encodeURIComponent(row.id)}>复核编译包 {row.id}</a>)}</nav>:null}
 {data?<><p className="muted">已检查 {data.items.length} / {data.total} 个命名技能包目录。来源不可读取的包不会被认定为匹配。</p>{data.items.some(row=>row.source_state==='unavailable')?<p className="muted">部分来源超出元数据检查上限，可用已保存 ID 打开这些技能包。</p>:null}<nav className="page-navigation" aria-label="技能包查询分页"><button className="button" disabled={busy||data.offset===0} onClick={()=>load(Math.max(0,data.offset-50))}>上一页技能包</button><button className="button" disabled={busy||data.next_offset===null} onClick={()=>load(data.next_offset!)}>下一页技能包</button></nav></>:null}<a href="#compile">打开已保存技能包 ID</a></section>
}
