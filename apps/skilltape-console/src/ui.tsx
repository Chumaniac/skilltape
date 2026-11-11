import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { getSkills, isSavedEvidenceId } from './api'
import type { Collection } from './types'

export function SkillPicker({ skillId, onOpen }: { skillId: string; onOpen: (id: string) => void }) {
  const [error, setError] = useState(skillId && !isSavedEvidenceId(skillId) ? "请填写技能包目录名称，不要填写路径。" : '')
  const input = useRef<HTMLInputElement>(null)
  const [catalogOpen, setCatalogOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [offset, setOffset] = useState(0)
  const [reload, setReload] = useState(0)
  const [catalog, setCatalog] = useState<Collection<string> | null>(null)
  const [catalogLoading, setCatalogLoading] = useState(false)
  const [catalogError, setCatalogError] = useState(false)
  useEffect(() => {
    if (!catalogOpen) return
    const controller = new AbortController()
    setCatalogLoading(true); setCatalogError(false); setCatalog(null)
    const timer = window.setTimeout(() => {
      getSkills(controller.signal, query, offset)
        .then(value => { if (!controller.signal.aborted) setCatalog(value) })
        .catch(() => { if (!controller.signal.aborted) setCatalogError(true) })
        .finally(() => { if (!controller.signal.aborted) setCatalogLoading(false) })
    }, 200)
    return () => { window.clearTimeout(timer); controller.abort() }
  }, [catalogOpen, query, offset, reload])
  return (
    <section className="panel receipt-picker" aria-label="打开已保存技能包"><form onSubmit={event => {
      event.preventDefault()
      const id = String(new FormData(event.currentTarget).get('skill-id') ?? '').trim()
      if (!isSavedEvidenceId(id)) {
        setError("请填写技能包目录名称，不要填写路径。")
        return
      }
      setError('')
      onOpen(id)
    }}>
      <label htmlFor="skill-id">已保存技能包 ID</label>
      <div className="receipt-picker-controls">
        <input ref={input} id="skill-id" name="skill-id" defaultValue={skillId} maxLength={255} required
          autoComplete="off" spellCheck={false} onInput={() => setError('')}
          aria-describedby="skill-picker-help skill-picker-error" aria-invalid={error ? true : undefined} />
        <button className="button" type="submit">打开技能包</button>
      </div>
      <p id="skill-picker-help" className="muted">输入 ID 或浏览下方已保存技能包。打开技能包不会运行它。</p>
      <p id="skill-picker-error" role="alert">{error}</p>
    </form>
    <details className="skill-catalog" open={catalogOpen} onToggle={event => setCatalogOpen(event.currentTarget.open)}>
      <summary>浏览已保存技能包</summary>
      {catalogOpen ? <div className="skill-catalog-content">
        <label htmlFor="skill-search">搜索已保存技能包</label>
        <input id="skill-search" type="search" maxLength={80} autoComplete="off" value={query}
          onChange={event => { setCatalogLoading(true); setQuery(event.target.value); setOffset(0) }} />
        {catalogLoading ? <p role="status">正在载入已保存技能包…</p> : catalogError ? <>
          <p role="alert">无法列出已保存技能包，请重试列表或在上方输入 ID。</p>
          <button className="button button-secondary" type="button" onClick={() => { setCatalogLoading(true); setReload(value => value + 1) }}>重新读取技能包列表</button>
        </> : catalog ? <>
          <p role="status">{catalog.total ? `显示 ${catalog.offset + 1}–${catalog.offset + catalog.items.length} / ${catalog.total} 个已保存技能包` : "没有符合搜索条件的技能包目录。"}</p>
          <ul className="skill-catalog-list" aria-label="已保存技能包">{catalog.items.map(id => <li key={id}>
            <button type="button" onClick={() => { if (input.current) input.current.value = id; setError(''); setCatalogOpen(false); onOpen(id) }}>{id}</button>
          </li>)}</ul>
          <nav className="receipt-picker-controls" aria-label="已保存技能包分页">
            <button className="button button-secondary" type="button" disabled={offset === 0} onClick={() => { setCatalogLoading(true); setOffset(Math.max(0, offset - 50)) }}>上一页已保存技能包</button>
            <button className="button button-secondary" type="button" disabled={catalog.next_offset === null} onClick={() => { setCatalogLoading(true); setOffset(catalog.next_offset ?? offset) }}>下一页已保存技能包</button>
          </nav>
          <p className="muted">名称来自当前工作区，打开时会检查包内容。</p>
        </> : null}
      </div> : null}
    </details></section>
  )
}

export function PageLoading({ label = "正在载入…" }: { label?: string }) {
  return (
    <div className="state-card" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      <p>{label}</p>
    </div>
  )
}

export function PageError({
  message,
  onRetry,
}: {
  message: string
  onRetry: () => void
}) {
  return (
    <div className="state-card state-card-error" role="alert">
      <span className="state-icon" aria-hidden="true">
        !
      </span>
      <div>
        <h2>暂时无法打开此页面</h2>
        <p>{message}</p>
        <button className="button button-secondary" type="button" onClick={onRetry}>
          重试
        </button>
      </div>
    </div>
  )
}

export function EmptyState({
  title,
  message,
  action,
}: {
  title: string
  message: string
  action?: ReactNode
}) {
  return (
    <div className="state-card state-card-empty">
      <div>
        <h2>{title}</h2>
        <p>{message}</p>
        {action}
      </div>
    </div>
  )
}

export function Metric({
  label,
  value,
  tone = 'neutral',
}: {
  label: string
  value: string
  tone?: 'neutral' | 'accent' | 'good' | 'warn' | 'danger'
}) {
  return (
    <div className={'metric metric-' + tone}>
      <span className="metric-label">{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

export function StatusBadge({
  children,
  tone = 'neutral',
}: {
  children: ReactNode
  tone?: 'neutral' | 'good' | 'warn' | 'danger'
}) {
  return <span className={'status-badge status-' + tone}>{children}</span>
}

export function CodeBlock({ value, label }: { value: string; label?: string }) {
  return (
    <div className="code-wrap">
      {label ? <span className="code-label">{label}</span> : null}
      <pre className="code-block">
        <code>{value}</code>
      </pre>
    </div>
  )
}

export function JsonDisclosure({
  label,
  value,
  open = false,
}: {
  label: string
  value: unknown
  open?: boolean
}) {
  const [expanded, setExpanded] = useState(open)
  const preview = useMemo(() => {
    if (!expanded) return ''
    const text = JSON.stringify(value, null, 2) ?? 'null'
    return text.length > 65536 ? text.slice(0, 65536) + "\n… 预览已截断，请查看已保存 JSON 获取完整记录。" : text
  }, [expanded, value])
  return (
    <details className="json-disclosure" open={expanded} onToggle={event => setExpanded(event.currentTarget.open)}>
      <summary>{label}</summary>
      {expanded && <CodeBlock value={preview} />}
    </details>
  )
}

const evidenceLabels: Readonly<Record<string, string>> = {
  succeeded: '成功', passed: '通过', failed: '失败', run_failed: '运行失败',
  assertion_failed: '断言失败', cancelled: '已取消', pending: '待执行', running: '运行中',
  blocked: '已阻断', unconfirmed: '尚未确认', unavailable: '暂不可用', unknown: '未知',
  preparation_failed: '准备失败', receipt_produced: '已生成凭据', 'not-run': '未执行',
  recorded: '已记录', unrecorded: '未记录', invalid: '无效', complete: '完整', open: '尚未结束',
  changed: '已变化', added: '新增', removed: '已移除', unchanged: '未变化', matched: '一致',
  different: '不同', allowed: '允许', denied: '拒绝', enabled: '已启用', disabled: '已关闭',
  file_exists: '文件存在', file_hash: '文件哈希', file_text_contains: '文件文本包含',
  command_exit: '命令退出码', step: '步骤', assertion: '断言', policy: '策略',
  preflight: '预检', execution: '执行', capture: '采集', compile: '编译', verify: '验证',
  low: '低', medium: '中', high: '高', critical: '严重',
  session_started: '会话开始', session_finished: '会话结束', terminal_command: '终端命令',
  filesystem_changed: '文件变化', permission_requested: '申请权限', permission_decided: '权限决策',
  environment_snapshot: '环境快照', capture_warning: '采集警告',
  cli: '命令行', shell: '终端', filesystem: '文件系统', permission: '权限',
  environment: '运行环境', system: '系统', redacted: '已脱敏', unredacted: '未脱敏',
  partially_redacted: '部分脱敏',
}

export function evidenceLabel(value: string | null): string {
  if (value === null) return '未记录'
  return evidenceLabels[value] ?? `未识别（${value}）`
}
