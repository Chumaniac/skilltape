import { PackageJourney } from './PackageJourney'
import { useEffect, useState } from 'react'

import { formatNumber, getSkillDiff, isSavedEvidenceId } from '../api'
import type { Permissions, SkillDiff } from '../types'
import { EmptyState, JsonDisclosure, Metric, PageError, PageLoading, SkillPicker, StatusBadge } from '../ui'

interface PermissionReviewProps {
  skillId?: string
}

export function PermissionReviewPage({ skillId = '' }: PermissionReviewProps) {
  const [skill, setSkill] = useState<SkillDiff | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)
  const [deniedOnly, setDeniedOnly] = useState(false)
  const [checkPage, setCheckPage] = useState(0)

  useEffect(() => {
    if (!isSavedEvidenceId(skillId)) return
    const controller = new AbortController()
    setLoading(true)
    setError('')
    getSkillDiff(skillId, controller.signal)
      .then((value) => {
        if (controller.signal.aborted) return
        setSkill(value)
        setDeniedOnly(false)
        setCheckPage(0)
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : "无法读取权限。")
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [reload, skillId])

  const picker = <SkillPicker key={skillId} skillId={skillId} onOpen={id => {
    if (id === skillId) setReload(value => value + 1)
    else window.location.hash = 'permissions?skill=' + encodeURIComponent(id)
  }} />
  if (isSavedEvidenceId(skillId) && loading) return <>{picker}<PageLoading label="正在载入权限复核…" /></>
  if (error) return <>{picker}<PageError message={error} onRetry={() => setReload((value) => value + 1)} /></>
  if (!skill) {
    return <>{picker}<EmptyState title="尚未选择技能包" message="打开已保存技能包，在运行前核对声明的权限。" /></>
  }

  const permissions = skill.permissions
  const risk = riskSummary(permissions)
  const checks = skill.policy_plan?.checks ?? []
  const deniedChecks = checks.filter((check) => !check.allowed)
  const matchingChecks = deniedOnly ? deniedChecks : checks
  const checkOffset = checkPage * 32
  return (
    <>{picker}<section className="page-section">
      <header className="page-heading">
        <div>
          <p className="eyebrow">权限复核</p>
          <h1>技能包权限</h1>
        </div>
        <StatusBadge tone={risk.tone}>{risk.label}</StatusBadge>
      </header>

      <PackageJourney skillId={skill.id} current="permissions" />
      <article className="panel" aria-labelledby="policy-preflight-title">
        <h2 id="policy-preflight-title">声明策略检查</h2>
        <p>仅检查声明策略，不运行步骤，也不生成验证凭据。输入、沙箱可用性和实际运行情况仍未验证。</p>
        {skill.policy_plan ? <>
          <StatusBadge tone={skill.policy_plan.allowed ? 'good' : 'danger'}>{skill.policy_plan.allowed ? "声明的策略允许" : "已阻断：请复核技能包"}</StatusBadge>
          {skill.policy_plan.package_error_codes.length > 0 && <p>技能包问题： {skill.policy_plan.package_error_codes.join(', ')}</p>}
          <details>
            <summary>检查 {skill.policy_plan.checks.length} 项策略检查</summary>
            <div className="receipt-filters" role="group" aria-label="策略检查筛选">
              <button className="button button-secondary" type="button" aria-pressed={!deniedOnly} onClick={() => { setDeniedOnly(false); setCheckPage(0) }}>全部检查（{checks.length})</button>
              <button className="button button-secondary" type="button" aria-pressed={deniedOnly} onClick={() => { setDeniedOnly(true); setCheckPage(0) }}>仅拒绝项（{deniedChecks.length})</button>
            </div>
            {matchingChecks.length === 0 ? <p role="status">{deniedOnly ? "此计划没有拒绝项。" : "此计划未记录策略检查。"}</p> : <>
              <p role="status">显示 {checkOffset + 1}–{Math.min(checkOffset + 32, matchingChecks.length)} / {matchingChecks.length} 项检查</p>
              <ol start={checkOffset + 1}>{matchingChecks.slice(checkOffset, checkOffset + 32).map(check => <li key={check.ordinal}>
                <code>{check.step_id}</code> · {check.allowed ? "允许" : "拒绝"} · <code>{check.code}</code><p>{check.reason}</p>
              </li>)}</ol>
              {matchingChecks.length > 32 && <nav className="page-navigation" aria-label="策略检查分页">
                <button className="button button-secondary" type="button" disabled={checkPage === 0} onClick={() => setCheckPage(value => Math.max(0, value - 1))}>上一页检查</button>
                <button className="button button-secondary" type="button" disabled={checkOffset + 32 >= matchingChecks.length} onClick={() => setCheckPage(value => value + 1)}>下一页检查</button>
              </nav>}
            </>}
            <p>完整计划： <code>skilltape preflight SKILL --json</code></p>
          </details>
        </> : <p>此技能包暂不支持预检。请使用当前 CLI，计划步骤和检查项均不能超过1,000条。</p>}
      </article>

      <div className="metric-grid">
        <Metric label="文件访问规则" value={formatNumber(permissions.filesystem.read.length + permissions.filesystem.write.length)} />
        <Metric label="可执行程序" value={formatNumber(permissions.process.executables.length)} />
        <Metric label="网络" value={permissions.network.enabled ? "已启用" : "关闭"} tone={permissions.network.enabled ? 'warn' : 'good'} />
        <Metric label="运行环境" value={permissions.secrets.read_environment ? "可读取" : "已阻断"} tone={permissions.secrets.read_environment ? 'danger' : 'good'} />
      </div>

      <div className="permission-grid">
        <PermissionCard
          eyebrow="文件系统"
          title="文件与目录"
          tone={permissions.filesystem.write.length > 0 ? 'warn' : 'good'}
          summary={permissions.filesystem.write.length + " 处可写范围"}
        >
          <PermissionList label="读取" values={permissions.filesystem.read} empty="未声明可读路径。" />
          <PermissionList label="写入" values={permissions.filesystem.write} empty="未声明可写路径。" />
        </PermissionCard>
        <PermissionCard
          eyebrow="进程"
          title="命令"
          tone={permissions.process.executables.length > 0 ? 'warn' : 'good'}
          summary={permissions.process.max_processes + " 进程上限"}
        >
          <PermissionList label="允许执行的程序" values={permissions.process.executables} empty="未声明可执行程序。" />
          <p className="permission-footnote">默认超时： {formatNumber(permissions.process.default_timeout_ms)} 毫秒</p>
        </PermissionCard>
        <PermissionCard
          eyebrow="网络"
          title="对外访问"
          tone={permissions.network.enabled ? 'danger' : 'good'}
          summary={permissions.network.enabled ? "已明确启用" : "默认关闭"}
        >
          {permissions.network.enabled ? (
            <PermissionList label="允许访问的主机" values={permissions.network.allow_hosts} empty="网络已启用，但没有配置主机条目。" />
          ) : (
            <div className="permission-safe">
              <span aria-hidden="true">✓</span>
              <span>此技能包没有网络访问权限。</span>
            </div>
          )}
        </PermissionCard>
        <PermissionCard
          eyebrow="敏感值"
          title="环境变量访问"
          tone={permissions.secrets.read_environment ? 'danger' : 'good'}
          summary={permissions.secrets.read_environment ? "需要复核" : "已阻断"}
        >
          <div className={permissions.secrets.read_environment ? 'permission-risk' : 'permission-safe'}>
            <span aria-hidden="true">{permissions.secrets.read_environment ? '!' : '✓'}</span>
            <span>
              {permissions.secrets.read_environment
                ? "此技能包可读取环境变量，请确认这符合预期。"
                : "未开放环境变量访问。"}
            </span>
          </div>
        </PermissionCard>
      </div>

      <article className="panel permission-source">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">来源约定</p>
            <h2>permissions.json</h2>
          </div>
          <span className="panel-note">{skill.package_path}</span>
        </div>
        <JsonDisclosure label="查看完整权限文档" value={permissions} open />
      </article>
    </section></>
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
    return { label: "需要复核", tone: 'danger' }
  }
  if (permissions.filesystem.write.length > 0 || permissions.process.executables.length > 0) {
    return { label: "限定范围的访问", tone: 'warn' }
  }
  return { label: "最小权限", tone: 'good' }
}
