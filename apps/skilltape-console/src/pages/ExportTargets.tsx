export function ExportTargetsPage() {
  const targets = [
    { id: 'generic', label: 'Generic', path: 'skill/', desc: 'Generic package layout' },
    { id: 'claude-code', label: 'Claude Code', path: '.claude/skills/<name>/', desc: 'Claude Code layout' },
    { id: 'codex', label: 'Codex', path: '.agents/skills/<name>/', desc: 'Codex layout' },
    { id: 'cursor', label: 'Cursor', path: '.cursor/skills/<name>/', desc: 'Cursor layout' },
  ]
  return (
    <div className="page">
      <h1 className="page-title">Export targets</h1>
      <p className="page-subtitle">Export the same Skill in a target layout. Each export is linted before writing.</p>
      <div className="card-grid">
        {targets.map((t) => (
          <div key={t.id} className="card">
            <div className="card-header">
              <strong>{t.label}</strong>
              <code>{t.id}</code>
            </div>
            <div className="card-body">
              <div className="mono">{t.path}</div>
              <p>{t.desc}</p>
            </div>
            <div className="card-footer">
              <code>skilltape export --target {t.id} ./my-skill --output ./out</code>
            </div>
          </div>
        ))}
      </div>
      <details className="callout"><summary>Export plugin interface</summary><p>External plugins must implement <code>EXPORT_REQUEST_SCHEMA_V1</code> and return <code>EXPORT_MANIFEST_SCHEMA_V1</code>. See <code>crates/skilltape-export/src/plugin.rs</code>.</p>
      </details>
    </div>
  )
}
