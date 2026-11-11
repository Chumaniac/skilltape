import { useEffect, useRef, useState } from 'react'
import { CodeBlock } from '../ui'

export function quotePath(value: string): string { return "'" + value.replaceAll("'", "'\\''") + "'" }
export function receiptName(skillId: string): string { return 'review-' + (skillId.replace(/[^a-zA-Z0-9_-]/g, '-').slice(0, 48) || 'skill') }
export function CommandCard({ label, command }: { label: string; command: string }) {
  const [result, setResult] = useState<{ value: string; message: string } | null>(null)
  const alive = useRef(true)
  useEffect(() => { alive.current = true; return () => { alive.current = false } }, [])
  return <div className="journey-command"><CodeBlock label={label} value={command} /><button type="button" className="button" onClick={async () => {
    try { await navigator.clipboard.writeText(command); if (alive.current) setResult({ value: command, message: 'Command copied. Nothing has run.' }) }
    catch { if (alive.current) setResult({ value: command, message: 'Copy unavailable. Select the command text and copy it manually.' }) }
  }}>Copy {label.toLowerCase()}</button><p className="muted" role="status">{result?.value === command ? result.message : ''}</p></div>
}
export function PackageJourney({ skillId, current }: { skillId: string; current: 'compile' | 'permissions' }) {
  const query = '?skill=' + encodeURIComponent(skillId)
  return <article className="panel package-journey" aria-labelledby="package-journey-title"><h2 id="package-journey-title">Continue with this package</h2><p>Review its permissions, verify it locally, then prepare the export. The package stays selected throughout.</p><nav className="journey-actions" aria-label="Package next steps">
    {current === 'permissions' ? <a className="button" href={'#compile' + query}>Return to package review</a> : <a className="button" href={'#permissions' + query}>Review this package’s permissions</a>}
    <a className="button" href={'#export' + query}>Prepare this package’s export</a>
  </nav><p className="muted">Lint and declared policy checks are separate from a successful verification Receipt.</p></article>
}
