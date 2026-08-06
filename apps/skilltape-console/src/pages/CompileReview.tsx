import { useEffect, useState } from 'react'

import { formatNumber, getSkillDiff } from '../api'
import type { SkillDiff } from '../types'
import { CodeBlock, EmptyState, JsonDisclosure, Metric, PageError, PageLoading, StatusBadge } from '../ui'

interface CompileReviewProps {
  skillId?: string
}

export function CompileReviewPage({ skillId = 'demo' }: CompileReviewProps) {
  const [skill, setSkill] = useState<SkillDiff | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)

  useEffect(() => {
    const controller = new AbortController()
    setLoading(true)
    setError('')
    getSkillDiff(skillId, controller.signal)
      .then(setSkill)
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : 'Could not read the Skill package.')
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [reload, skillId])

  if (loading) return <PageLoading label="Loading compile review…" />
  if (error) return <PageError message={error} onRetry={() => setReload((value) => value + 1)} />
  if (!skill) {
    return (
      <EmptyState
        title="No Skill selected"
        message="Choose a compiled Skill package to inspect its provenance and lint result."
      />
    )
  }

  const stepCount = skill.workflow.steps?.length ?? 0
  const lintTone = skill.lint.errors.length === 0 ? 'good' : 'danger'
  return (
    <section className="page-section">
      <header className="page-heading">
        <div>
          <p className="eyebrow">Compile review</p>
          <h1>Inspect what the compiler produced</h1>
          <p className="page-description">
            Review the executable Workflow, package evidence, and lint diagnostics before sharing a Skill.
          </p>
        </div>
        <StatusBadge tone={lintTone}>
          {skill.lint.errors.length === 0 ? 'Lint clean' : skill.lint.errors.length + ' errors'}
        </StatusBadge>
      </header>

      <div className="metric-grid">
        <Metric label="Skill" value={skill.id} tone="accent" />
        <Metric label="Workflow steps" value={formatNumber(stepCount)} />
        <Metric label="Files checked" value={formatNumber(skill.lint.files_checked)} />
        <Metric
          label="Warnings"
          value={formatNumber(skill.lint.warnings.length)}
          tone={skill.lint.warnings.length > 0 ? 'warn' : 'good'}
        />
      </div>

      <div className="review-grid">
        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Executable contract</p>
              <h2>Workflow</h2>
            </div>
            <span className="panel-note">Read-only</span>
          </div>
          {stepCount > 0 ? (
            <ol className="workflow-list">
              {skill.workflow.steps?.map((step, index) => (
                <li key={String(step.id ?? index)}>
                  <span className="step-index">{String(index + 1).padStart(2, '0')}</span>
                  <div className="step-copy">
                    <strong>{String(step.id ?? 'Unnamed step')}</strong>
                    <span>{String(step.action ?? 'unknown action')}</span>
                  </div>
                  <code>{String(step.program ?? step.path ?? step.operation ?? 'structured step')}</code>
                </li>
              ))}
            </ol>
          ) : (
            <EmptyState
              title="No workflow steps"
              message="This package has a valid workflow document, but it contains no executable steps."
            />
          )}
          <JsonDisclosure label="View workflow JSON" value={skill.workflow} />
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Package identity</p>
              <h2>{skill.package_path}</h2>
            </div>
          </div>
          <div className="file-list">
            {skill.files.map((file) => (
              <div className="file-row" key={file.path}>
                <span className="file-dot" aria-hidden="true" />
                <div className="file-copy">
                  <strong>{file.path}</strong>
                  <span>{formatNumber(file.bytes)} bytes</span>
                </div>
                <code title={file.sha256}>{file.sha256.slice(0, 10)}…</code>
              </div>
            ))}
          </div>
        </article>

        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">Diagnostics</p>
              <h2>Lint evidence</h2>
            </div>
            <span className="panel-note">{formatNumber(skill.lint.errors.length + skill.lint.warnings.length)} findings</span>
          </div>
          {skill.lint.errors.length === 0 && skill.lint.warnings.length === 0 ? (
            <div className="success-line" role="status">
              <span className="success-mark" aria-hidden="true">
                ✓
              </span>
              <span>Schema, paths, permissions, and lockfile checks are clean.</span>
            </div>
          ) : (
            <ul className="diagnostic-list">
              {[...skill.lint.errors, ...skill.lint.warnings].map((diagnostic) => (
                <li className={'diagnostic diagnostic-' + diagnostic.level} key={diagnostic.code + diagnostic.file}>
                  <StatusBadge tone={diagnostic.level === 'error' ? 'danger' : 'warn'}>{diagnostic.level}</StatusBadge>
                  <div>
                    <strong>{diagnostic.code}</strong>
                    <span>{diagnostic.message}</span>
                    <code>{diagnostic.file}{diagnostic.path ? ':' + diagnostic.path : ''}</code>
                  </div>
                </li>
              ))}
            </ul>
          )}
          <JsonDisclosure label="View manifest JSON" value={skill.manifest} />
        </article>
      </div>
    </section>
  )
}
