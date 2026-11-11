import { useEffect, useState } from 'react'
import { apiBaseLabel } from './api'
type Identity = { schema: string; version: string; sourceRevision: string | null; dirty: boolean; channel: string; capabilities: string[] }
declare const __SKILLTAPE_BUILD__: Identity
const ui = __SKILLTAPE_BUILD__
const labels: Record<string,string> = { 'local-starters/v1':'Local starters', 'verification-batches/v1':'Verification batches', 'verify-attempts/v1':'Attempt history', 'receipt-comparison/v1':'Receipt comparisons', 'readonly-console/v1':'Read-only Console' }
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
  return <details className="build-identity" onToggle={event => setOpen(event.currentTarget.open)}><summary>Installation details · {ui.channel === 'release' ? 'release' : 'source candidate'}</summary>
    <p role="status">{state === 'matched' ? 'Console and local API match this source.' : state === 'checking' ? 'Checking local API source…' : state === 'mismatch' ? 'Source differs or includes local changes. Inspect both builds before using their capabilities.' : 'Local API identity could not be checked. No capability match is confirmed.'}</p>
    <p>v{ui.version} · {ui.sourceRevision?.slice(0, 12) ?? 'source unknown'}{ui.dirty ? ' · local changes' : ''}</p>
    <p>Capabilities: {ui.capabilities.map(id => labels[id] ?? 'Unrecognized capability').join(', ')}</p>
    <p>Checked once for this page. Reload after replacing an installation.</p>
    <p>Execution needs a supported native sandbox. Windows replay and Verify remain unavailable. Build identity does not approve execution or authenticate a Receipt.</p>
  </details>
}
