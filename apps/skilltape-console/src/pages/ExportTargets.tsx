import { useEffect, useRef, useState } from 'react'
import { getReceipt, getSkillDiff, isSavedEvidenceId } from '../api'
import type { Receipt, SkillDiff } from '../types'
import { EmptyState, PageError, PageLoading, SkillPicker } from '../ui'
import { CommandCard, quotePath, receiptName } from './PackageJourney'
const targets = [
  { id: 'generic', label: 'Generic', path: 'skill/', description: 'A portable package without an agent-specific directory.' },
  { id: 'claude-code', label: 'Claude Code', path: '.claude/skills/<name>/', description: 'A package laid out for Claude Code.' },
  { id: 'codex', label: 'Codex', path: '.agents/skills/<name>/', description: 'A package laid out for Codex.' },
  { id: 'cursor', label: 'Cursor', path: '.cursor/skills/<name>/', description: 'A package laid out for Cursor.' },
] as const
export function ExportTargetsPage({ skillId = '', receiptId = '' }: { skillId?: string; receiptId?: string }) {
  const [skill, setSkill] = useState<SkillDiff | null>(null), [loading, setLoading] = useState(Boolean(skillId)), [error, setError] = useState(''), [reload, setReload] = useState(0)
  const [targetId, setTargetId] = useState('generic'), [output, setOutput] = useState<string | null>(null), [includeReceipt, setIncludeReceipt] = useState(Boolean(receiptId)), [receiptInput, setReceiptInput] = useState(receiptId), [saved, setSaved] = useState<{id:string; receipt:Receipt} | null>(null), [receiptError, setReceiptError] = useState(''), [checking, setChecking] = useState(false)
  const [verificationId] = useState(() => receiptName(skillId) + '-' + crypto.randomUUID().slice(0, 8))
  const active = useRef<AbortController | null>(null)
  useEffect(() => {
    if (!isSavedEvidenceId(skillId)) return
    const controller = new AbortController(); setLoading(true); setError('')
    getSkillDiff(skillId, controller.signal).then(value => { if (!controller.signal.aborted) setSkill(value) }).catch(reason => { if (!controller.signal.aborted) setError(reason instanceof Error ? reason.message : 'Could not read this package.') }).finally(() => { if (!controller.signal.aborted) setLoading(false) })
    return () => controller.abort()
  }, [skillId, reload])
  useEffect(() => () => active.current?.abort(), [])
  const picker = <SkillPicker key={skillId} skillId={skillId} onOpen={id => { if (id === skillId) setReload(value => value + 1); else window.location.hash = 'export?skill=' + encodeURIComponent(id) }} />
  if (loading) return <>{picker}<PageLoading label="Loading the selected package…" /></>
  if (error) return <>{picker}<PageError message={error} onRetry={() => setReload(value => value + 1)} /></>
  if (!skill) return <>{picker}<EmptyState title="Choose a package to export" message="Open a saved package from Compile review, or enter its saved ID above. Its real path will be carried into the commands." /></>
  const target = targets.find(value => value.id === targetId) ?? targets[0], name = verificationId, receiptPath = '.skilltape/receipts/' + name + '.json'
  const destination = output ?? './out/' + target.id + '/' + name
  const validOutput = destination.trim().length > 0 && destination.length <= 4096 && !/[\u0000-\u001f\u007f]/.test(destination)
  const ready = validOutput && (!includeReceipt || saved?.id === receiptInput && saved.receipt.status === 'succeeded')
  const verify = `skilltape verify ${quotePath(skill.package_path)} --receipt ${quotePath(receiptPath)} --json`
  const command = `skilltape export --target ${target.id} ${quotePath(skill.package_path)} --output ${quotePath(destination)}${includeReceipt && saved ? ' --receipt ' + quotePath('.skilltape/receipts/' + saved.id + '.json') : ''}`
  return <>{picker}<section className="page-section export-workbench"><header className="page-heading"><div><h1>Prepare this package for use</h1><p className="page-description">Selected package: <code>{skill.package_path}</code>. Run these commands from the workspace directory opened by this Console. Paths are relative to it; nothing runs in this view.</p></div></header>
    <article className="panel"><h2>1. Verify locally and keep the Receipt</h2><p>Review the package and permissions first. Verification executes the Skill using the CLI’s policies; this page does not run it.</p><div role="region" aria-label="Verification command"><CommandCard label="Verification command" command={verify} /></div><a className="button" href={'#receipt?run=' + encodeURIComponent(name) + '&skill=' + encodeURIComponent(skill.id)}>Open the saved verification Receipt</a><p className="muted">Open it after the command finishes. A missing or failed Receipt remains an explicit result, not a successful verification.</p></article>
    <div className="export-layout"><article className="panel export-options"><h2>2. Choose where the package will go</h2><label htmlFor="export-target">Export target</label><select id="export-target" name="export-target" value={target.id} onChange={event => setTargetId(event.target.value)}>{targets.map(value => <option key={value.id} value={value.id}>{value.label}</option>)}</select><p>{target.description}</p><dl><dt>Layout inside the output</dt><dd><code>{target.path}</code></dd></dl><label htmlFor="export-output">New output directory</label><input id="export-output" name="export-output" autoComplete="off" spellCheck={false} value={destination} onChange={event => setOutput(event.target.value)} aria-invalid={!validOutput} /><p className="muted">Choose a new directory. Existing output is never replaced.</p>{!validOutput ? <p role="alert">Use one directory path without control characters.</p> : null}
      <label className="export-receipt-choice"><input type="checkbox" checked={includeReceipt} onChange={event => setIncludeReceipt(event.target.checked)} />Include a saved Receipt</label>{includeReceipt ? <div><label htmlFor="export-receipt">Saved Receipt ID</label><input id="export-receipt" name="export-receipt" autoComplete="off" spellCheck={false} value={receiptInput} onChange={event => { active.current?.abort(); setChecking(false); setReceiptInput(event.target.value.replace(/\.json$/i, '')); setSaved(null); setReceiptError('') }} /><button type="button" className="button" disabled={checking} onClick={async () => {
        if (!isSavedEvidenceId(receiptInput)) { setReceiptError('Use a saved filename, not a path.'); return }
        active.current?.abort(); const controller = new AbortController(); active.current = controller; setChecking(true); setReceiptError('')
        try { const value = await getReceipt(receiptInput, controller.signal); if (!controller.signal.aborted) { setSaved({id: receiptInput, receipt:value.document}); if (value.document.status !== 'succeeded') setReceiptError('This Receipt did not succeed. Inspect it before preparing an export with evidence.') } }
        catch(reason) { if (!controller.signal.aborted) { setSaved(null); setReceiptError(reason instanceof Error ? reason.message : 'Could not read the Receipt.') } }
        finally { if (active.current === controller) { active.current = null; setChecking(false) } }
      }}>{checking ? 'Reading Receipt…' : 'Read saved Receipt'}</button><p role="status">{receiptError || (saved ? 'Receipt read. The export CLI will check its binding to this package before writing.' : 'Read a saved Receipt to include its real path.')}</p></div> : null}
    </article><article className="panel export-command"><h2>3. Copy the export command</h2>{ready ? <div role="region" aria-label="Export command"><CommandCard label="Export command" command={command} /></div> : <p role="status">Complete the output path and optional Receipt checks above to prepare the command.</p>}<p>Run the command, then inspect its delivery result. Selecting a target or reading a Receipt does not activate an installation.</p><a href={'#compile?skill=' + encodeURIComponent(skill.id)}>Return to this package’s review</a></article></div>
  </section></>
}
