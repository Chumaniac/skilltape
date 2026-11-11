import { PackageJourney } from './PackageJourney'
import { useEffect, useState } from 'react'

import { formatNumber, getSkillDiff, isSavedEvidenceId } from '../api'
import type { SkillDiff } from '../types'
import { CodeBlock, EmptyState, JsonDisclosure, Metric, PageError, PageLoading, SkillPicker, StatusBadge } from '../ui'

interface CompileReviewProps {
  skillId?: string
}

export function CompileReviewPage({ skillId = '' }: CompileReviewProps) {
  const [skill, setSkill] = useState<SkillDiff | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)
  const [workflowPage, setWorkflowPage] = useState(0)
  const [diagnosticPage, setDiagnosticPage] = useState(0)

  useEffect(() => {
    setSkill(null)
    setWorkflowPage(0)
    setDiagnosticPage(0)
    if (!isSavedEvidenceId(skillId)) { setLoading(false); setError(''); return }
    const controller = new AbortController()
    setLoading(true)
    setError('')
    getSkillDiff(skillId, controller.signal)
      .then(value => { if (!controller.signal.aborted) setSkill(value) })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : "无法读取技能包。")
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [reload, skillId])

  const picker = <SkillPicker key={skillId} skillId={skillId} onOpen={id => {
    if (id === skillId) setReload(value => value + 1)
    else window.location.hash = 'compile?skill=' + encodeURIComponent(id)
  }} />
  if (isSavedEvidenceId(skillId) && loading) return <>{picker}<PageLoading label="正在载入编译复核…" /></>
  if (error) return <>{picker}<PageError message={error} onRetry={() => setReload((value) => value + 1)} /></>
  if (!skill) {
    return (
      <>{picker}<EmptyState
        title="尚未选择技能包"
        message="打开已保存技能包，复核工作流、文件和静态检查结果。"
      /></>
    )
  }

  const stepCount = skill.workflow.steps?.length ?? 0
  const pageSize = 100
  const diagnostics = [...skill.lint.errors, ...skill.lint.warnings]
  const lintTone = skill.lint.errors.length === 0 ? 'good' : 'danger'
  return (
    <>{picker}<section className="page-section">
      <header className="page-heading">
        <div>
          <p className="eyebrow">编译复核</p>
          <h1>已编译技能包</h1>
        </div>
        <StatusBadge tone={lintTone}>
          {skill.lint.errors.length === 0 ? "静态检查通过" : skill.lint.errors.length + ' 项错误'}
        </StatusBadge>
      </header>

      <div className="metric-grid">
        <Metric label="技能包" value={skill.id} tone="accent" />
        <Metric label="工作流步骤" value={formatNumber(stepCount)} />
        <Metric label="已检查文件" value={formatNumber(skill.lint.files_checked)} />
        <Metric
          label="警告"
          value={formatNumber(skill.lint.warnings.length)}
          tone={skill.lint.warnings.length > 0 ? 'warn' : 'good'}
        />
      </div>

      <article className="panel compile-source" aria-labelledby="compile-source-title">
        <h2 id="compile-source-title">采集来源</h2>
        {skill.compile_source ? <>
          <p>记录的来源： {formatNumber(skill.compile_source.event_count)} 个事件。请将这些摘要与保留的采集记录对比。</p>
          <p>文件清单 <code>{skill.compile_source.manifest_sha256}</code></p>
          <p>事件 <code>{skill.compile_source.events_sha256}</code></p>
          <p className="panel-note">仅比较规范化v1内容摘要，不认证作者身份，也不证明重放成功。</p>
        </> : <p>此技能包未记录采集来源，已有工作流与验证证据仍可查看。</p>}
      </article>

      <PackageJourney skillId={skill.id} current="compile" />
      <div className="review-grid">
        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">可执行程序约定</p>
              <h2>工作流</h2>
            </div>
            <span className="panel-note">只读</span>
          </div>
          {stepCount > 0 ? (
            <ol className="workflow-list">
              {skill.workflow.steps?.slice(workflowPage * pageSize, (workflowPage + 1) * pageSize).map((step, index) => (
                <li key={String(step.id ?? '') + ':' + (workflowPage * pageSize + index)}>
                  <span className="step-index">{String(workflowPage * pageSize + index + 1).padStart(2, '0')}</span>
                  <div className="step-copy">
                    <strong>{String(step.id ?? "未命名步骤")}</strong>
                    <span>{String(step.action ?? "未知操作")}</span>
                  </div>
                  <code>{String(step.program ?? step.path ?? step.operation ?? "结构化步骤")}</code>
                </li>
              ))}
            </ol>
          ) : (
            <EmptyState
              title="没有工作流步骤"
              message="此技能包的工作流文档有效，但没有可执行步骤。"
            />
          )}
          {stepCount > pageSize && <nav aria-label="工作流分页" className="panel-heading">
            <button type="button" disabled={workflowPage === 0} onClick={() => setWorkflowPage(value => value - 1)}>上一页工作流</button>
            <span>步骤 {workflowPage * pageSize + 1}–{Math.min((workflowPage + 1) * pageSize, stepCount)} / {stepCount}</span>
            <button type="button" disabled={(workflowPage + 1) * pageSize >= stepCount} onClick={() => setWorkflowPage(value => value + 1)}>下一页工作流</button>
          </nav>}
          <JsonDisclosure label="查看工作流 JSON" value={skill.workflow} />
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">技能包身份</p>
              <h2>{skill.package_path}</h2>
            </div>
          </div>
          <div className="file-list">
            {skill.files.map((file) => (
              <div className="file-row" key={file.path}>
                <span className="file-dot" aria-hidden="true" />
                <div className="file-copy">
                  <strong>{file.path}</strong>
                  <span>{formatNumber(file.bytes)} 字节</span>
                </div>
                <code title={file.sha256}>{file.sha256.slice(0, 10)}…</code>
              </div>
            ))}
          </div>
        </article>

        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">诊断</p>
              <h2>静态检查证据</h2>
            </div>
            <span className="panel-note">{formatNumber(skill.lint.errors.length + skill.lint.warnings.length)} 项问题</span>
          </div>
          {skill.lint.errors.length === 0 && skill.lint.warnings.length === 0 ? (
            <div className="success-line" role="status">
              <span className="success-mark" aria-hidden="true">
                ✓
              </span>
              <span>结构、路径、权限和锁文件检查均通过。</span>
            </div>
          ) : (
            <ul className="diagnostic-list">
              {diagnostics.slice(diagnosticPage * pageSize, (diagnosticPage + 1) * pageSize).map((diagnostic, index) => (
                <li className={'diagnostic diagnostic-' + diagnostic.level} key={diagnostic.code + diagnostic.file + ':' + (diagnosticPage * pageSize + index)}>
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
          {diagnostics.length > pageSize && <nav aria-label="诊断分页" className="panel-heading">
            <button type="button" disabled={diagnosticPage === 0} onClick={() => setDiagnosticPage(value => value - 1)}>上一页诊断</button>
            <span>发现的问题 {diagnosticPage * pageSize + 1}–{Math.min((diagnosticPage + 1) * pageSize, diagnostics.length)} / {diagnostics.length}</span>
            <button type="button" disabled={(diagnosticPage + 1) * pageSize >= diagnostics.length} onClick={() => setDiagnosticPage(value => value + 1)}>下一页诊断</button>
          </nav>}
          <JsonDisclosure label="查看清单 JSON" value={skill.manifest} />
        </article>
      </div>
    </section></>
  )
}
