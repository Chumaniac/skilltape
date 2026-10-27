import { useState } from 'react'
import { CodeBlock } from '../ui'

const targets = [
  { id: 'generic', label: 'Generic', path: 'skill/', description: 'A portable package without an agent-specific directory.' },
  { id: 'claude-code', label: 'Claude Code', path: '.claude/skills/<name>/', description: 'A package laid out for Claude Code.' },
  { id: 'codex', label: 'Codex', path: '.agents/skills/<name>/', description: 'A package laid out for Codex.' },
  { id: 'cursor', label: 'Cursor', path: '.cursor/skills/<name>/', description: 'A package laid out for Cursor.' },
] as const

export function ExportTargetsPage() {
  const [targetId, setTargetId] = useState<string>('generic')
  const [includeReceipt, setIncludeReceipt] = useState(false)
  const target = targets.find(item => item.id === targetId) ?? targets[0]
  const command = `skilltape export --target ${target.id} ./my-skill --output ./out/${target.id}${includeReceipt ? ' --receipt ./receipt.json' : ''}`
  return <section className="page-section export-workbench">
    <header className="page-heading"><div><h1>Export guide</h1><p className="page-description">Choose a layout, then run the matching command locally. Export checks the saved package before writing files.</p></div></header>
    <div className="export-layout"><article className="panel export-options"><h2>Choose the package layout</h2>
      <div className="select-control"><label htmlFor="export-target">Export target</label><select id="export-target" name="export-target" value={target.id} onChange={event => setTargetId(event.target.value)}>{targets.map(item => <option key={item.id} value={item.id}>{item.label}</option>)}</select></div>
      <p>{target.description}</p><dl><dt>Layout inside the output</dt><dd><code>{target.path}</code></dd></dl>
      <label className="export-receipt-choice"><input type="checkbox" name="include-receipt" checked={includeReceipt} onChange={event => setIncludeReceipt(event.target.checked)} />Include a saved Receipt</label>
      <p className="muted">Optional. The CLI reads the supplied Receipt and checks whether it can be included; selecting this option does not verify it here.</p>
    </article><article className="panel export-command"><h2>Run from your terminal</h2>
      <div role="region" aria-label="Export command"><CodeBlock label="Replace the example package, output and Receipt paths" value={command} /></div>
      <p>Choose an output location permitted by the CLI. Review its result before using the exported package.</p>
      <p className="muted">This Console view only prepares command text. It does not write an export or run the Skill.</p>
    </article></div>
    <details className="panel export-plugin"><summary>Developing another export target</summary><p>External plugins implement <code>EXPORT_REQUEST_SCHEMA_V1</code> and return <code>EXPORT_MANIFEST_SCHEMA_V1</code>. The interface is defined in <code>crates/skilltape-export/src/plugin.rs</code>.</p></details>
  </section>
}
