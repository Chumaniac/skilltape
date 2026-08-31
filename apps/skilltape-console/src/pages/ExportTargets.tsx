export function ExportTargetsPage() {
  const targets = [
    { id: 'generic', label: 'Generic', path: 'skill/', desc: 'Deterministic generic package' },
    { id: 'claude-code', label: 'Claude Code', path: '.claude/skills/<name>/', desc: 'Claude Code layout' },
    { id: 'codex', label: 'Codex', path: '.agents/skills/<name>/', desc: 'Codex layout' },
    { id: 'cursor', label: 'Cursor', path: '.cursor/skills/<name>/', desc: 'Cursor layout' },
  ]
  return (
    <div className="page">
      <h1 className="page-title">Export targets</h1>
      <p className="page-subtitle">Deterministic exporters — same package hash, different layouts. All exports are linted before writing.</p>
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
      <div className="callout">
        <strong>Plugin contract:</strong> external plugins must implement <code>EXPORT_REQUEST_SCHEMA_V1</code> and return <code>EXPORT_MANIFEST_SCHEMA_V1</code>. See <code>crates/skilltape-export/src/plugin.rs</code>.
      </div>
    </div>
  )
}
