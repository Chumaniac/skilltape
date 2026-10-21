import { useEffect, useState } from 'react'

import { formatNumber, getSkillDiff } from '../api'
import type { Permissions, SkillDiff } from '../types'
import { EmptyState, JsonDisclosure, Metric, PageError, PageLoading, StatusBadge } from '../ui'

interface PermissionReviewProps {
  skillId?: string
}

export function PermissionReviewPage({ skillId = 'demo' }: PermissionReviewProps) {
  const [skill, setSkill] = useState<SkillDiff | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)
  const [deniedOnly, setDeniedOnly] = useState(false)
  const [checkPage, setCheckPage] = useState(0)

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setError('')
    getSkillDiff(skillId, controller.signal)
      .then((value) => {
        setSkill(value)
        setDeniedOnly(false)
        setCheckPage(0)
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : 'Could not read permissions.')
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [reload, skillId])

  if (loading) return <PageLoading label="Loading permission review…" />
  if (error) return <PageError message={error} onRetry={() => setReload((value) => value + 1)} />
  if (!skill) {
    return <EmptyState title="No Skill selected" message="Select a Skill package to inspect its permission contract." />
  }

  const permissions = skill.permissions
  const risk = riskSummary(permissions)
  const checks = skill.policy_plan?.checks ?? []
  const deniedChecks = checks.filter((check) => !check.allowed)
  const matchingChecks = deniedOnly ? deniedChecks : checks
  const checkOffset = checkPage * 32
  return (
    <section className="page-section">
      <header className="page-heading">
        <div>
          <p className="eyebrow">Permission review</p>
          <h1>Know what the Skill is allowed to touch</h1>
          <p className="page-description">
            Permissions are explicit, inspectable, and separate from the prose in SKILL.md.
          </p>
        </div>
        <StatusBadge tone={risk.tone}>{risk.label}</StatusBadge>
      </header>

      <article className="panel" aria-labelledby="policy-preflight-title">
        <h2 id="policy-preflight-title">Execution policy preflight</h2>
        <p>Checks declared steps with the replay policy. No steps have run and no Receipt is created. Input data, sandbox availability and runtime results remain unverified.</p>
        {skill.policy_plan ? <>
          <StatusBadge tone={skill.policy_plan.allowed ? 'good' : 'danger'}>{skill.policy_plan.allowed ? 'Allowed by declared policy' : 'Blocked — review package'}</StatusBadge>
          {skill.policy_plan.package_error_codes.length > 0 && <p>Package findings: {skill.policy_plan.package_error_codes.join(', ')}</p>}
          <details>
            <summary>Inspect {skill.policy_plan.checks.length} policy checks</summary>
            <div className="receipt-filters" role="group" aria-label="Policy check filter">
              <button className="button button-secondary" type="button" aria-pressed={!deniedOnly} onClick={() => { setDeniedOnly(false); setCheckPage(0) }}>All checks ({checks.length})</button>
              <button className="button button-secondary" type="button" aria-pressed={deniedOnly} onClick={() => { setDeniedOnly(true); setCheckPage(0) }}>Denied only ({deniedChecks.length})</button>
            </div>
            {matchingChecks.length === 0 ? <p role="status">No denied checks in this plan.</p> : <>
              <p role="status">Showing {checkOffset + 1}–{Math.min(checkOffset + 32, matchingChecks.length)} of {matchingChecks.length} checks</p>
              <ol start={checkOffset + 1}>{matchingChecks.slice(checkOffset, checkOffset + 32).map(check => <li key={check.ordinal}>
                <code>{check.step_id}</code> · {check.allowed ? 'Allowed' : 'Denied'} · <code>{check.code}</code><p>{check.reason}</p>
              </li>)}</ol>
              {matchingChecks.length > 32 && <nav className="page-navigation" aria-label="Policy checks pagination">
                <button className="button button-secondary" type="button" disabled={checkPage === 0} onClick={() => setCheckPage(value => Math.max(0, value - 1))}>Previous checks</button>
                <button className="button button-secondary" type="button" disabled={checkOffset + 32 >= matchingChecks.length} onClick={() => setCheckPage(value => value + 1)}>Next checks</button>
              </nav>}
            </>}
            <p>Filtering changes this view only. Use <code>skilltape preflight SKILL --json</code> for the full bounded plan.</p>
          </details>
        </> : <p>Preflight is unavailable for this package. Use the current CLI and keep the plan within 1,000 steps and checks.</p>}
      </article>

      <div className="metric-grid">
        <Metric label="Filesystem rules" value={formatNumber(permissions.filesystem.read.length + permissions.filesystem.write.length)} />
        <Metric label="Executables" value={formatNumber(permissions.process.executables.length)} />
        <Metric label="Network" value={permissions.network.enabled ? 'Enabled' : 'Off'} tone={permissions.network.enabled ? 'warn' : 'good'} />
        <Metric label="Environment" value={permissions.secrets.read_environment ? 'Readable' : 'Blocked'} tone={permissions.secrets.read_environment ? 'danger' : 'good'} />
      </div>

      <div className="permission-grid">
        <PermissionCard
          eyebrow="Filesystem"
          title="Files & directories"
          tone={permissions.filesystem.write.length > 0 ? 'warn' : 'good'}
          summary={permissions.filesystem.write.length + ' write scope(s)'}
        >
          <PermissionList label="Read" values={permissions.filesystem.read} empty="No read paths declared." />
          <PermissionList label="Write" values={permissions.filesystem.write} empty="No write paths declared." />
        </PermissionCard>
        <PermissionCard
          eyebrow="Process"
          title="Commands"
          tone={permissions.process.executables.length > 0 ? 'warn' : 'good'}
          summary={permissions.process.max_processes + ' max process'}
        >
          <PermissionList label="Allowed executables" values={permissions.process.executables} empty="No executables declared." />
          <p className="permission-footnote">Default timeout: {formatNumber(permissions.process.default_timeout_ms)} ms</p>
        </PermissionCard>
        <PermissionCard
          eyebrow="Network"
          title="Outbound access"
          tone={permissions.network.enabled ? 'danger' : 'good'}
          summary={permissions.network.enabled ? 'Explicitly enabled' : 'Disabled by default'}
        >
          {permissions.network.enabled ? (
            <PermissionList label="Allowed hosts" values={permissions.network.allow_hosts} empty="Network is enabled without host entries." />
          ) : (
            <div className="permission-safe">
              <span aria-hidden="true">✓</span>
              <span>No network access is available to this Skill.</span>
            </div>
          )}
        </PermissionCard>
        <PermissionCard
          eyebrow="Secrets"
          title="Environment access"
          tone={permissions.secrets.read_environment ? 'danger' : 'good'}
          summary={permissions.secrets.read_environment ? 'Review required' : 'Blocked'}
        >
          <div className={permissions.secrets.read_environment ? 'permission-risk' : 'permission-safe'}>
            <span aria-hidden="true">{permissions.secrets.read_environment ? '!' : '✓'}</span>
            <span>
              {permissions.secrets.read_environment
                ? 'The Skill can read environment variables. Confirm this is intentional.'
                : 'Environment variables are not available.'}
            </span>
          </div>
        </PermissionCard>
      </div>

      <article className="panel permission-source">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Source contract</p>
            <h2>permissions.json</h2>
          </div>
          <span className="panel-note">{skill.package_path}</span>
        </div>
        <JsonDisclosure label="View complete permission document" value={permissions} open />
      </article>
    </section>
  )
}

function PermissionCard({
  eyebrow,
  title,
  tone,
  summary,
  children,
}: {
  eyebrow: string
  title: string
  tone: 'good' | 'warn' | 'danger'
  summary: string
  children: React.ReactNode
}) {
  return (
    <article className={'permission-card permission-card-' + tone}>
      <div className="permission-card-heading">
        <div>
          <p className="eyebrow">{eyebrow}</p>
          <h2>{title}</h2>
        </div>
        <StatusBadge tone={tone}>{summary}</StatusBadge>
      </div>
      {children}
    </article>
  )
}

function PermissionList({ label, values, empty }: { label: string; values: string[]; empty: string }) {
  return (
    <div className="permission-list">
      <span className="list-label">{label}</span>
      {values.length > 0 ? (
        <ul>
          {values.map((value) => (
            <li key={value}>
              <code>{value}</code>
            </li>
          ))}
        </ul>
      ) : (
        <span className="muted">{empty}</span>
      )}
    </div>
  )
}

function riskSummary(permissions: Permissions): { label: string; tone: 'good' | 'warn' | 'danger' } {
  if (permissions.secrets.read_environment || permissions.network.enabled) {
    return { label: 'Review required', tone: 'danger' }
  }
  if (permissions.filesystem.write.length > 0 || permissions.process.executables.length > 0) {
    return { label: 'Scoped access', tone: 'warn' }
  }
  return { label: 'Least privilege', tone: 'good' }
}
