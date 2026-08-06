import { useEffect, useState } from 'react'
import { createRoot } from 'react-dom/client'

import { apiBaseLabel } from './api'
import { CompileReviewPage } from './pages/CompileReview'
import { PermissionReviewPage } from './pages/PermissionReview'
import { ReceiptViewPage } from './pages/ReceiptView'
import { TimelinePage } from './pages/Timeline'
import './styles.css'

type PageKey = 'timeline' | 'compile' | 'permissions' | 'receipt'

interface RouteState {
  page: PageKey
  skillId?: string
  receiptId?: string
  tapeId?: string
}

function readRoute(): RouteState {
  const hash = window.location.hash.replace(/^#/, '') || 'timeline'
  const [pageValue, query] = hash.split('?')
  const page: PageKey =
    pageValue === 'compile' || pageValue === 'permissions' || pageValue === 'receipt'
      ? pageValue
      : 'timeline'
  const params = new URLSearchParams(query)
  return {
    page,
    skillId: params.get('skill') ?? undefined,
    receiptId: params.get('run') ?? undefined,
    tapeId: params.get('tape') ?? undefined,
  }
}

function App() {
  const [route, setRoute] = useState<RouteState>(readRoute)

  useEffect(() => {
    const handleHashChange = () => setRoute(readRoute())
    window.addEventListener('hashchange', handleHashChange)
    return () => window.removeEventListener('hashchange', handleHashChange)
  }, [])

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a className="brand" href="#timeline" translate="no">
          <span className="brand-mark" aria-hidden="true">
            <span />
            <span />
            <span />
          </span>
          <span>
            <strong>SkillTape</strong>
            <small>LOCAL CONSOLE</small>
          </span>
        </a>
        <div className="sidebar-rule" />
        <nav aria-label="Console sections">
          <p className="nav-label">Inspect</p>
          <NavLink href="#timeline" active={route.page === 'timeline'} label="Capture timeline" icon="◌" />
          <NavLink href="#compile?skill=demo" active={route.page === 'compile'} label="Compile review" icon="⌁" />
          <NavLink href="#permissions?skill=demo" active={route.page === 'permissions'} label="Permissions" icon="⊙" />
          <NavLink href="#receipt?run=run-a" active={route.page === 'receipt'} label="Verify Receipt" icon="✓" />
        </nav>
        <div className="sidebar-bottom">
          <div className="local-card">
            <span className="live-dot" aria-hidden="true" />
            <div>
              <strong>Local only</strong>
              <span>No cloud sync</span>
            </div>
          </div>
          <p className="sidebar-footnote">Evidence stays on this machine.</p>
        </div>
      </aside>

      <div className="main-shell">
        <header className="topbar">
          <div className="topbar-context">
            <span className="topbar-kicker">Workspace / default</span>
            <span className="topbar-separator">/</span>
            <span>{pageLabel(route.page)}</span>
          </div>
          <div className="api-status" aria-label="Local API endpoint">
            <span className="api-status-dot" aria-hidden="true" />
            <span>Local API {apiBaseLabel()}</span>
          </div>
        </header>

        <main id="main-content" className="main-content">
          {route.page === 'timeline' ? <TimelinePage tapeId={route.tapeId} /> : null}
          {route.page === 'compile' ? <CompileReviewPage skillId={route.skillId} /> : null}
          {route.page === 'permissions' ? <PermissionReviewPage skillId={route.skillId} /> : null}
          {route.page === 'receipt' ? <ReceiptViewPage receiptId={route.receiptId} /> : null}
        </main>

        <footer className="app-footer">
          <span>SkillTape Console <span translate="no">v0.1</span></span>
          <span>Read-only evidence surface</span>
        </footer>
      </div>
    </div>
  )
}

function NavLink({
  href,
  active,
  label,
  icon,
}: {
  href: string
  active: boolean
  label: string
  icon: string
}) {
  return (
    <a className={'nav-link' + (active ? ' nav-link-active' : '')} href={href} aria-current={active ? 'page' : undefined}>
      <span className="nav-icon" aria-hidden="true">{icon}</span>
      <span>{label}</span>
    </a>
  )
}

function pageLabel(page: PageKey): string {
  if (page === 'compile') return 'Compile review'
  if (page === 'permissions') return 'Permission review'
  if (page === 'receipt') return 'Verify Receipt'
  return 'Capture timeline'
}

createRoot(document.getElementById('root')!).render(
  <App />,
)
