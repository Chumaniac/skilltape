import { useEffect, useState } from 'react'
import { createRoot } from 'react-dom/client'

import { apiBaseLabel, isSavedEvidenceId } from './api'
import { CompileReviewPage } from './pages/CompileReview'
import { ExportTargetsPage } from './pages/ExportTargets'
import { PermissionReviewPage } from './pages/PermissionReview'
import { ReceiptViewPage } from './pages/ReceiptView'
import { TimelinePage } from './pages/Timeline'
import { DeliveriesPage } from './pages/Deliveries'
import { BatchReviewPage } from './pages/BatchReview'
import { LocalTaskPage } from './pages/LocalTask'
import metadata from '../package.json'
import { BuildIdentity } from './BuildIdentity'
import './styles.css'
import mark from './assets/skilltape.svg'
import { RenderBoundary } from './RenderBoundary'

type PageKey = 'task' | 'timeline' | 'compile' | 'permissions' | 'receipt' | 'export' | 'deliveries' | 'batch' | 'not-found'

interface RouteState {
  page: PageKey
  skillId?: string
  receiptId?: string
  tapeId?: string
  tapeOffset: number
  eventOffset: number
  deliveryId?: string
  deliveryOffset: number
}

function readOffset(value: string | null): number {
  const offset = Number(value)
  return Number.isSafeInteger(offset) && offset >= 0 ? offset : 0
}

function readRoute(): RouteState {
  const native = (window as Window & { __NATIVE_WORKSPACE__?: boolean }).__NATIVE_WORKSPACE__ === true
  const hash = window.location.hash.replace(/^#/, '') || (native ? 'task' : 'timeline')
  const [pageValue, query] = hash.split('?')
  const page: PageKey =
    pageValue === 'task' || pageValue === 'timeline' || pageValue === 'compile' || pageValue === 'permissions' || pageValue === 'receipt' || pageValue === 'export' || pageValue === 'deliveries' || pageValue === 'batch'
      ? pageValue
      : 'not-found'
  const params = new URLSearchParams(query)
  return {
    page,
    skillId: params.get('skill') ?? undefined,
    receiptId: params.get('run') ?? undefined,
    tapeId: params.get('tape') ?? undefined,
    tapeOffset: readOffset(params.get('tapesOffset')),
    eventOffset: readOffset(params.get('eventsOffset')),
    deliveryId: params.get('delivery') ?? undefined,
    deliveryOffset: readOffset(params.get('deliveryOffset')),
  }
}

function App() {
  const [route, setRoute] = useState<RouteState>(readRoute)
  const skillQuery = route.skillId && isSavedEvidenceId(route.skillId) ? '?skill=' + encodeURIComponent(route.skillId) : ''

  useEffect(() => {
    const handleHashChange = () => setRoute(readRoute())
    window.addEventListener('hashchange', handleHashChange)
    return () => window.removeEventListener('hashchange', handleHashChange)
  }, [])

  return (
    <><a className="skip-link" href="#main-content" onClick={event => {
      event.preventDefault()
      document.getElementById('main-content')?.focus()
    }}>跳转到内容</a><div className="app-shell">
      <aside className="sidebar">
        <a className="brand" href="#timeline" translate="no" onClick={protectDraftNavigation}>
          <img src={mark} width="32" height="32" alt="" />
          <span>
            <strong>SkillTape</strong>
            <small>证据控制台</small>
          </span>
        </a>
        <div className="sidebar-rule" />
        <nav aria-label="控制台导航">
          <p className="nav-label">操作</p><NavLink href="#task" active={route.page === 'task'} label="核对数量" /><p className="nav-label">检查</p>
          <NavLink href="#batch" active={route.page === 'batch'} label="验证批次" />
          <NavLink href="#deliveries" active={route.page === 'deliveries'} label="交付工作台" />
          <NavLink href="#timeline" active={route.page === 'timeline'} label="采集时间线" />
          <NavLink href={'#compile' + skillQuery} active={route.page === 'compile'} label="编译复核" />
          <NavLink href={'#permissions' + skillQuery} active={route.page === 'permissions'} label="权限" />
          <NavLink href="#receipt" active={route.page === 'receipt'} label="查看验证凭据" />
          <p className="nav-label" style={{ marginTop: 16 }}>导出</p>
          <NavLink href={'#export' + skillQuery} active={route.page === 'export'} label="导出指南" />
        </nav>
        <div className="sidebar-bottom">
          <div className="local-card">
            <div>
              <strong>已保存本机证据</strong>
              <span>只读检查</span>
            </div>
          </div>
        </div>
      </aside>

      <div className="main-shell">
        <header className="topbar">
          <div className="topbar-context">
            <span className="topbar-kicker">本机证据</span>
            <span className="topbar-separator">/</span>
            <span>{pageLabel(route.page)}</span>
          </div>
          <details className="api-status connection-details" aria-label="本机 API 地址">
            <summary>本地工作区</summary><code>{apiBaseLabel()}</code>
          </details>
        </header>

        <main id="main-content" className="main-content" tabIndex={-1}>
          {route.page !== 'task' && <details className="console-start-guide"><summary>这个工作台用来做什么？如何开始</summary><p>这里查看已经保存在本机的运行与交付证据，适合复查一次操作用了哪些输入、生成了哪些文件，以及验证记录还缺什么。</p><ol><li>尚无记录时，先按安装文档用 CLI 完成采集；不要把本地数量计算当成运行证明。</li><li>从“采集时间线”或“验证批次”打开记录，再核对编译、权限与交付。</li><li>确认结果与范围后，查看验证凭据并准备导出。查看记录不会重新执行 Skill。</li></ol><p><a href="#timeline" onClick={protectDraftNavigation}>查看已采集的记录</a> · <a href="https://github.com/Chumaniac/skilltape#快速开始">阅读本机使用方法</a></p></details>}

          {route.page === 'not-found' ? <section className="page-section"><header className="page-heading"><div><h1>页面不存在</h1><p className="page-description">此链接没有对应的控制台页面，请从导航选择功能或返回采集记录。</p></div></header><p><a className="button" href="#timeline">打开采集时间线</a></p></section> : null}
          {route.page === 'task' ? <LocalTaskPage /> : null}
          {route.page === 'batch' ? <BatchReviewPage /> : null}
          {route.page === 'deliveries' ? <DeliveriesPage deliveryId={route.deliveryId} offset={route.deliveryOffset} /> : null}
          {route.page === 'timeline' ? <TimelinePage tapeId={route.tapeId} tapeOffset={route.tapeOffset} eventOffset={route.eventOffset} /> : null}
          {route.page === 'compile' ? <CompileReviewPage key={route.skillId ?? ''} skillId={route.skillId} /> : null}
          {route.page === 'permissions' ? <PermissionReviewPage key={route.skillId ?? ''} skillId={route.skillId} /> : null}
          {route.page === 'receipt' ? <ReceiptViewPage key={route.receiptId ?? ''} receiptId={route.receiptId} skillId={route.skillId} /> : null}
          {route.page === 'export' ? <ExportTargetsPage key={route.skillId ?? ''} skillId={route.skillId} receiptId={route.receiptId} /> : null}
        </main>

        <BuildIdentity />
        <footer className="app-footer">
          <span>SkillTape 控制台 <span translate="no">v{metadata.version}</span></span>
          <span>本机计算 · 已保存证据保持只读</span>
        </footer>
      </div>
    </div></>
  )
}

function protectDraftNavigation(event: React.MouseEvent<HTMLAnchorElement>) {
  if (event.currentTarget.hash === window.location.hash || (readRoute().page === 'task' && event.currentTarget.hash === '#task')) return
  const leaving = new Event('beforeunload', { cancelable: true })
  if (!window.dispatchEvent(leaving) && !window.confirm("放弃未保存的本地计算并打开其他页面？")) event.preventDefault()
}

function NavLink({
  href,
  active,
  label,
}: {
  href: string
  active: boolean
  label: string
}) {
  return (
    <a className={'nav-link' + (active ? ' nav-link-active' : '')} href={href} aria-current={active ? 'page' : undefined} onClick={protectDraftNavigation}>
      <span>{label}</span>
    </a>
  )
}

function pageLabel(page: PageKey): string {
  if (page === 'task') return "核对数量"
  if (page === 'not-found') return "页面不存在"
  if (page === 'batch') return "验证批次"
  if (page === 'deliveries') return "交付工作台"
  if (page === 'compile') return "编译复核"
  if (page === 'permissions') return "权限复核"
  if (page === 'receipt') return "查看验证凭据"
  if (page === 'export') return "导出指南"
  return "采集时间线"
}

createRoot(document.getElementById('root')!).render(
  <RenderBoundary><App /></RenderBoundary>,
)
