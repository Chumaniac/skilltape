import {SavedReceipts} from './SavedReceipts'
import { useEffect, useRef, useState } from 'react'
import { getReceipt, getSkillDiff, isSavedEvidenceId } from '../api'
import type { Receipt, SkillDiff } from '../types'
import { EmptyState, PageError, PageLoading, SkillPicker } from '../ui'
import { verificationPreparation } from '../verification-preparation'
import { CommandCard, quotePath } from './PackageJourney'
const targets = [
  { id: 'generic', label: "通用", path: 'skill/', description: "通用可携带包，不包含特定工具的目录结构。" },
  { id: 'claude-code', label: 'Claude Code', path: '.claude/skills/<name>/', description: "适用于 Claude Code 的目录结构。" },
  { id: 'codex', label: 'Codex', path: '.agents/skills/<name>/', description: "适用于 Codex 的目录结构。" },
  { id: 'cursor', label: 'Cursor', path: '.cursor/skills/<name>/', description: "适用于 Cursor 的目录结构。" },
] as const
export function ExportTargetsPage({ skillId = '', receiptId = '' }: { skillId?: string; receiptId?: string }) {
  const [skill, setSkill] = useState<SkillDiff | null>(null), [loading, setLoading] = useState(Boolean(skillId)), [error, setError] = useState(''), [reload, setReload] = useState(0)
  const [targetId, setTargetId] = useState('generic'), [output, setOutput] = useState<string | null>(null), [includeReceipt, setIncludeReceipt] = useState(Boolean(receiptId)), [receiptInput, setReceiptInput] = useState(receiptId), [saved, setSaved] = useState<{id:string; receipt:Receipt} | null>(null), [receiptError, setReceiptError] = useState(''), [checking, setChecking] = useState(false)
  const [preparation, setPreparation] = useState(() => isSavedEvidenceId(skillId) ? verificationPreparation(skillId) : {id:'',retained:false})
  const active = useRef<AbortController | null>(null)
  useEffect(() => {
    if (!isSavedEvidenceId(skillId)) return
    const controller = new AbortController(); setLoading(true); setError('')
    getSkillDiff(skillId, controller.signal).then(value => { if (!controller.signal.aborted) setSkill(value) }).catch(reason => { if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : "无法读取此技能包。") }).finally(() => { if (!controller.signal.aborted) setLoading(false) })
    return () => controller.abort()
  }, [skillId, reload])
  useEffect(() => () => active.current?.abort(), [])
  async function readSavedReceipt(id:string){
    if (!isSavedEvidenceId(id)) { setReceiptError("请填写已保存文件名，不要填写路径。"); return }
    active.current?.abort(); const controller=new AbortController();active.current=controller;setChecking(true);setReceiptError('');setReceiptInput(id);setIncludeReceipt(true);setSaved(null)
    try {const value=await getReceipt(id,controller.signal);if(!controller.signal.aborted){setSaved({id,receipt:value.document});if(value.document.status!=='succeeded')setReceiptError("此凭据未成功，请先检查，再准备附带证据的导出。")}}
    catch(reason){if(!controller.signal.aborted)setReceiptError(reason instanceof Error?reason.message:"无法读取凭据，请在验证完成后重试。")}
    finally{if(active.current===controller){active.current=null;setChecking(false)}}
  }
  const picker = <SkillPicker key={skillId} skillId={skillId} onOpen={id => { if (id === skillId) setReload(value => value + 1); else window.location.hash = 'export?skill=' + encodeURIComponent(id) }} />
  if (loading) return <>{picker}<PageLoading label="正在载入所选技能包…" /></>
  if (error) return <>{picker}<PageError message={error} onRetry={() => setReload(value => value + 1)} /></>
  if (!skill) return <>{picker}<EmptyState title="选择要导出的技能包" message="从编译复核打开已保存技能包，或在上方输入 ID。命令中会使用该包的实际路径。" /></>
  const target = targets.find(value => value.id === targetId) ?? targets[0], name = preparation.id, receiptPath = '.skilltape/receipts/' + name + '.json'
  const destination = output ?? './out/' + target.id + '/' + name
  const validOutput = destination.trim().length > 0 && destination.length <= 4096 && !/[\u0000-\u001f\u007f]/.test(destination)
  const ready = validOutput && (!includeReceipt || saved?.id === receiptInput && saved.receipt.status === 'succeeded')
  const verify = `skilltape verify ${quotePath(skill.package_path)} --receipt ${quotePath(receiptPath)} --json`
  const command = `skilltape export --target ${target.id} ${quotePath(skill.package_path)} --output ${quotePath(destination)}${includeReceipt && saved ? ' --receipt ' + quotePath('.skilltape/receipts/' + saved.id + '.json') : ''}`
  return <>{picker}<section className="page-section export-workbench"><header className="page-heading"><div><h1>准备使用此技能包</h1><p className="page-description">所选技能包： <code>{skill.package_path}</code>。请在控制台打开的工作目录中执行以下命令。路径均相对于该目录；本页面不会执行命令。</p></div></header>
    <article className="panel"><h2>1. 本机验证并保留凭据</h2><p>先复核技能包及权限。验证会按 CLI 策略执行技能包，本页面不会运行它。</p><div role="region" aria-label="验证命令"><CommandCard label="验证命令" command={verify} /></div><div className="journey-actions"><button type="button" className="button" disabled={checking} onClick={()=>void readSavedReceipt(name)}>检查本次验证凭据</button>{saved?.id===name?<a className="button" href={'#receipt?run='+encodeURIComponent(name)+'&skill='+encodeURIComponent(skill.id)}>查看本次验证凭据</a>:null}<button type="button" className="button" disabled={checking} onClick={()=>{active.current?.abort();setPreparation(verificationPreparation(skillId,true));setReceiptInput('');setIncludeReceipt(false);setSaved(null);setReceiptError('');setOutput(null)}}>准备新的验证</button></div><p className="muted">此标签页最多保留凭据文件名24小时，刷新或切换复核页面后仍可使用。命令结束后检查已保存结果；准备新的验证会使用新文件名，不覆盖旧结果。</p>{!preparation.retained?<p role="alert">此标签页无法保留凭据文件名，请保存已复制命令，或在下方输入凭据 ID。</p>:null}{receiptInput===name?<p role="status">{receiptError||(saved?"凭据已读取，导出 CLI 会检查它与技能包的绑定关系。":"命令结束后再读取凭据。")}</p>:null}</article>
    <div className="export-layout"><article className="panel export-options"><h2>2. 选择导出位置</h2><SavedReceipts onRead={id=>void readSavedReceipt(id)}/><label htmlFor="export-target">导出目标</label><select id="export-target" name="export-target" value={target.id} onChange={event => setTargetId(event.target.value)}>{targets.map(value => <option key={value.id} value={value.id}>{value.label}</option>)}</select><p>{target.description}</p><dl><dt>输出目录结构</dt><dd><code>{target.path}</code></dd></dl><label htmlFor="export-output">新建输出目录</label><input id="export-output" name="export-output" autoComplete="off" spellCheck={false} value={destination} onChange={event => setOutput(event.target.value)} aria-invalid={!validOutput} /><p className="muted">请选择新目录，不会覆盖已有输出。</p>{!validOutput ? <p role="alert">请填写单个目录路径，不要包含控制字符。</p> : null}
      <label className="export-receipt-choice"><input type="checkbox" checked={includeReceipt} onChange={event => setIncludeReceipt(event.target.checked)} />附带已保存凭据</label>{includeReceipt ? <div><label htmlFor="export-receipt">已保存凭据 ID</label><input id="export-receipt" name="export-receipt" autoComplete="off" spellCheck={false} value={receiptInput} onChange={event => { active.current?.abort(); setChecking(false); setReceiptInput(event.target.value.replace(/\.json$/i, '')); setSaved(null); setReceiptError('') }} /><button type="button" className="button" disabled={checking} onClick={()=>void readSavedReceipt(receiptInput)}>{checking ? "正在读取凭据…" : "读取已保存凭据"}</button><p role="status">{receiptError || (saved ? "凭据已读取，导出 CLI 会在写入前检查它与技能包的绑定关系。" : "读取已保存凭据，将其实际路径加入导出命令。")}</p></div> : null}
    </article><article className="panel export-command"><h2>3. 复制导出命令</h2>{ready ? <div role="region" aria-label="导出命令"><CommandCard label="导出命令" command={command} /></div> : <p role="status">填写输出路径并完成可选凭据检查后，即可生成命令。</p>}<p>执行命令后检查交付结果。选择目标或读取凭据不会激活安装。</p><a href={'#compile?skill=' + encodeURIComponent(skill.id)}>返回此技能包的复核</a></article></div>
  </section></>
}
