import { Component, type ReactNode } from 'react'

export class RenderBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false }
  static getDerivedStateFromError() { return { failed: true } }
  render() {
    if (this.state.failed) return <section role="alert" className="state-card">
      <h1>页面未能显示</h1><p>请重新载入。已保存的记录可在重新打开后查看。</p>
      <button onClick={() => window.location.reload()}>重新载入</button>
    </section>
    return this.props.children
  }
}
