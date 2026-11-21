import { useState } from 'react'
import { createRoot } from 'react-dom/client'
import { LocalTaskPage } from '../src/pages/LocalTask'
import { RenderBoundary } from '../src/RenderBoundary'
import mark from '../src/assets/skilltape.svg'
import '../src/styles.css'
import './try.css'

const themeKey = 'skilltape.sample.theme'
type Theme = 'light' | 'dark'
// Storage only remembers the appearance; private windows may refuse it.
function storedTheme(): Theme | null {
  try {
    const value = localStorage.getItem(themeKey)
    return value === 'light' || value === 'dark' ? value : null
  } catch { return null }
}
const systemTheme = (): Theme => window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'

const initialTheme = storedTheme()
if (initialTheme) document.documentElement.dataset.theme = initialTheme

function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(initialTheme ?? systemTheme())
  function toggle() {
    const next: Theme = theme === 'dark' ? 'light' : 'dark'
    document.documentElement.dataset.theme = next
    try { localStorage.setItem(themeKey, next) } catch { /* appearance stays for this visit */ }
    setTheme(next)
  }
  return <button type="button" className="trial-icon-button" onClick={toggle} aria-label={theme === 'dark' ? '切换到浅色外观' : '切换到深色外观'}>
    <svg className="trial-sun" viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" /></svg>
    <svg className="trial-moon" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" /></svg>
  </button>
}

createRoot(document.getElementById('root')!).render(<RenderBoundary>
  <div className="trial-bar"><header className="trial-nav"><img src={mark} width="24" height="24" alt="" /><strong>SkillTape</strong>
    <div className="trial-actions">
      <ThemeToggle />
      <a href="https://github.com/Chumaniac/skilltape" rel="noopener" aria-label="源码与本机安装（GitHub）"><span>源码与本机安装</span>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7h10v10M7 17 17 7" /></svg></a>
    </div></header></div>
  <main id="main-content" className="trial-content"><LocalTaskPage /></main>
  <footer className="trial-footer"><p>数量只在当前标签页计算，不上传。命令录制、重放与 Receipt 在本机 CLI 中完成。
    <a href="https://github.com/Chumaniac/skilltape#快速开始" rel="noopener">从一份对账单开始</a></p></footer>
</RenderBoundary>)
