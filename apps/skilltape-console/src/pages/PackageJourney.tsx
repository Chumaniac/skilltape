import { useEffect, useRef, useState } from 'react'
import { CodeBlock } from '../ui'

export function quotePath(value: string): string { return "'" + value.replaceAll("'", "'\\''") + "'" }
export function receiptName(skillId: string): string { return 'review-' + (skillId.replace(/[^a-zA-Z0-9_-]/g, '-').slice(0, 48) || 'skill') }
export function CommandCard({ label, command }: { label: string; command: string }) {
  const [result, setResult] = useState<{ value: string; message: string } | null>(null)
  const alive = useRef(true)
  useEffect(() => { alive.current = true; return () => { alive.current = false } }, [])
  return <div className="journey-command"><CodeBlock label={label} value={command} /><button type="button" className="button" onClick={async () => {
    try { await navigator.clipboard.writeText(command); if (alive.current) setResult({ value: command, message: "命令已复制，尚未执行。" }) }
    catch { if (alive.current) setResult({ value: command, message: "无法自动复制，请选中命令文本手动复制。" }) }
  }}>复制 {label.toLowerCase()}</button><p className="muted" role="status">{result?.value === command ? result.message : ''}</p></div>
}
export function PackageJourney({ skillId, current }: { skillId: string; current: 'compile' | 'permissions' }) {
  const query = '?skill=' + encodeURIComponent(skillId)
  return <article className="panel package-journey" aria-labelledby="package-journey-title"><h2 id="package-journey-title">继续处理此技能包</h2><p>先复核权限，再在本机验证，最后准备导出。过程中会保留已选择的技能包。</p><nav className="journey-actions" aria-label="技能包下一步">
    {current === 'permissions' ? <a className="button" href={'#compile' + query}>返回技能包复核</a> : <a className="button" href={'#permissions' + query}>复核此技能包权限</a>}
    <a className="button" href={'#export' + query}>准备导出此技能包</a>
  </nav><p className="muted">静态检查和声明策略检查，与成功验证凭据是不同的结果。</p></article>
}
