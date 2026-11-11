import {useEffect, useRef, useState} from 'react'
import {getReceipts, formatDate, type ReceiptFilters, type ReceiptSummary} from '../api'
import type {Collection} from '../types'

const emptyFilters: ReceiptFilters = {status: '', skillHash: ''}

export function SavedReceipts({onRead, onCompare}: {onRead: (id: string) => void; onCompare?: (side: 'before' | 'after', id: string) => void}) {
  const [data, setData] = useState<Collection<ReceiptSummary> | null>(null)
  const [draft, setDraft] = useState(emptyFilters)
  const [applied, setApplied] = useState(emptyFilters)
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const aborter = useRef<AbortController | null>(null)
  useEffect(() => () => { aborter.current?.abort(); aborter.current = null }, [])

  async function load(offset = 0, filters = applied) {
    const next = {...filters, skillHash: filters.skillHash.trim().toLowerCase()}
    if (!/^[a-f0-9]{0,64}$/.test(next.skillHash)) {
      setError('Use up to 64 hexadecimal characters for the fingerprint.')
      return
    }
    aborter.current?.abort()
    const controller = new AbortController()
    aborter.current = controller
    setBusy(true)
    setError('')
    try {
      const value = await getReceipts(controller.signal, offset, next)
      if (!controller.signal.aborted) {
        setData(value)
        setApplied(next)
        setDraft(next)
      }
    } catch (reason) {
      if (!controller.signal.aborted) {
        setError((reason instanceof Error ? reason.message : 'Could not list saved Receipts.') + (data ? ' Previous results and filters are kept.' : ''))
        setDraft(applied)
      }
    } finally {
      if (aborter.current === controller) { aborter.current = null; setBusy(false) }
    }
  }

  return <section className="saved-receipts" aria-label="Saved Receipt library">
    <button type="button" className="button" disabled={busy} onClick={() => load(data?.offset || 0)}>{busy ? 'Reading Receipt list…' : data ? 'Refresh saved Receipts' : 'Browse saved Receipts'}</button>
    <form onSubmit={event => {event.preventDefault(); void load(0, draft)}}>
      <fieldset className="saved-receipt-filters" disabled={busy}>
        <legend>Find saved Receipts</legend>
        <label>Recorded Receipt status<select value={draft.status} onChange={event => setDraft({...draft, status: event.target.value as ReceiptFilters['status']})}>
          <option value="">All recorded statuses</option><option value="succeeded">Succeeded</option><option value="run_failed">Run failed</option><option value="assertion_failed">Assertion failed</option><option value="cancelled">Cancelled</option><option value="unavailable">Metadata unavailable</option>
        </select></label>
        <label>Skill fingerprint prefix<input value={draft.skillHash} maxLength={64} spellCheck={false} autoComplete="off" onChange={event => setDraft({...draft, skillHash: event.target.value})}/></label>
        <button type="submit" className="button">Apply Receipt filters</button>
        <button type="button" className="button" onClick={() => load(0, emptyFilters)}>Clear Receipt filters</button>
      </fieldset>
    </form>
    <p className="muted">Search reads local metadata only. Unavailable metadata cannot be matched to a fingerprint. Read the full Receipt before using it with the current package.</p>
    <p role="status">{error || (!data ? '' : data.total ? `${data.total} matching Receipt files. Recorded status is not execution or export approval.` : 'No saved Receipt files match these filters.')}</p>
    {data ? <><ul className="saved-receipt-list">{data.items.map(row => <li key={row.id}>
      <div><strong>{row.id}</strong><span>{row.metadata_state === 'available' ? row.status : 'Read to inspect; metadata unavailable'}{row.modified_at_ms ? ' · File updated ' + formatDate(row.modified_at_ms) : ''}</span>{row.skill_hash ? <code>Package fingerprint {row.skill_hash.slice(0, 16)}…</code> : null}</div>
      <div className="receipt-row-actions"><button type="button" className="button" disabled={busy} aria-label={'Read Receipt ' + row.id} onClick={() => onRead(row.id)}>Read</button>
        {onCompare ? <><button type="button" className="button" disabled={busy} aria-label={'Use ' + row.id + ' as earlier Receipt'} onClick={() => onCompare('before', row.id)}>Use as earlier</button><button type="button" className="button" disabled={busy} aria-label={'Use ' + row.id + ' as later Receipt'} onClick={() => onCompare('after', row.id)}>Use as later</button></> : null}</div>
    </li>)}</ul><nav className="page-navigation" aria-label="Saved Receipt pagination">
      <button className="button" disabled={busy || data.offset === 0} onClick={() => load(Math.max(0, data.offset - 50))}>Previous Receipts</button>
      <button className="button" disabled={busy || data.next_offset === null} onClick={() => load(data.next_offset!)}>Next Receipts</button>
    </nav></> : null}
  </section>
}
