import { createRoot } from 'react-dom/client'
import { LocalTaskPage } from '../src/pages/LocalTask'
import { RenderBoundary } from '../src/RenderBoundary'
import mark from '../src/assets/skilltape.svg'
import '../src/styles.css'
import './try.css'

createRoot(document.getElementById('root')!).render(<RenderBoundary>
  <header className="trial-nav"><img src={mark} width="32" height="32" alt="" /><strong>SkillTape</strong>
    <a href="https://github.com/Chumaniac/skilltape">源码与本机安装 ↗</a></header>
  <main id="main-content" className="trial-content"><LocalTaskPage /></main>
</RenderBoundary>)
