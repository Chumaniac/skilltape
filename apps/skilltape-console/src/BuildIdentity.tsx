import { useEffect, useState } from 'react'
import { apiBaseLabel } from './api'
type Identity = { schema: string; version: string; sourceRevision: string | null; dirty: boolean; channel: string; capabilities: string[] }
declare const __SKILLTAPE_BUILD__: Identity
const ui = __SKILLTAPE_BUILD__
const labels: Record<string,string> = { 'local-starters/v1':"本机示例任务", 'verification-batches/v1':"验证批次", 'verify-attempts/v1':"尝试记录", 'receipt-comparison/v1':"凭据对比", 'readonly-console/v1':"只读控制台" }
export function BuildIdentity() {
  const [open, setOpen] = useState(false)
  const [state, setState] = useState<'checking' | 'matched' | 'mismatch' | 'unavailable'>('checking')
  useEffect(() => {
    if (!open) return
    setState('checking')
    const controller = new AbortController()
    const timer = window.setTimeout(() => { controller.abort(); setState('unavailable') }, 10000)
    fetch(apiBaseLabel() + '/build-info', { signal: controller.signal, cache: 'no-store' }).then(async response => {
      if (!response.ok || Number(response.headers.get('content-length') ?? 0) > 16384) throw new Error('unavailable')
      const raw = await response.text(); if (raw.length > 16384) throw new Error('unavailable')
      const api: unknown = JSON.parse(raw)
      if (controller.signal.aborted) return
      const equal = typeof api === 'object' && api !== null && ['schema', 'version', 'sourceRevision', 'dirty', 'channel', 'capabilities'].every(key => JSON.stringify((api as Record<string, unknown>)[key]) === JSON.stringify(ui[key as keyof Identity]))
      setState(equal && ui.sourceRevision !== null && !ui.dirty ? 'matched' : 'mismatch')
    }).catch(() => { if (!controller.signal.aborted) setState('unavailable') }).finally(() => window.clearTimeout(timer))
    const leave = () => { controller.abort(); window.clearTimeout(timer); setState('unavailable') }
    window.addEventListener('pagehide', leave)
    return () => { controller.abort(); window.clearTimeout(timer); window.removeEventListener('pagehide', leave) }
  }, [open])
  return <details className="build-identity" onToggle={event => setOpen(event.currentTarget.open)}><summary>安装详情 · {ui.channel === 'release' ? '正式版本' : "源码候选版本"}</summary>
    <p role="status">{state === 'matched' ? "控制台与本机 API 的源码版本一致。" : state === 'checking' ? "正在核对本机 API 来源…" : state === 'mismatch' ? "源码不同或包含本地改动，使用前请先核对两个构建版本。" : "无法核对本机 API 身份，尚未确认能力一致。"}</p>
    <p>v{ui.version} · {ui.sourceRevision?.slice(0, 12) ?? "源码未知"}{ui.dirty ? " · 本地改动" : ''}</p>
    <p>可用能力： {ui.capabilities.map(id => labels[id] ?? "无法识别的能力").join(', ')}</p>
    <p>本页面仅检查一次；更换安装版本后请重新载入。</p>
    <p>执行需要系统支持的原生沙箱。Windows 暂不支持重放与验证。版本一致不代表批准执行或认证凭据来源。</p>
  </details>
}
