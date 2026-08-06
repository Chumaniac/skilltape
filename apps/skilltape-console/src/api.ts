import type {
  Collection,
  SkillDiff,
  StoredDocument,
  TapeEvent,
  TapeEvents,
  TapeSummary,
  WorkspaceSummary,
} from './types'

const configuredBase = (import.meta.env.VITE_SKILLTAPE_API_BASE as string | undefined) ?? '/api/v1'
const API_BASE = configuredBase.replace(/\/+$/, '')

export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(message: string, status: number, code = 'request_failed') {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  const response = await fetch(API_BASE + path, {
    headers: { Accept: 'application/json' },
    signal,
  })
  const payload: unknown = await response.json().catch(() => null)
  if (!response.ok) {
    const error =
      payload && typeof payload === 'object' && 'error' in payload
        ? (payload as { error?: { message?: string; code?: string } }).error
        : undefined
    throw new ApiError(
      error?.message ?? 'The local API returned ' + response.status + '.',
      response.status,
      error?.code,
    )
  }
  return payload as T
}

export function getWorkspaces(signal?: AbortSignal) {
  return get<Collection<WorkspaceSummary>>('/workspaces', signal)
}

export function getTapes(workspaceId: string, signal?: AbortSignal) {
  return get<Collection<TapeSummary>>(
    '/workspaces/' + encodeURIComponent(workspaceId) + '/tapes?limit=50',
    signal,
  )
}

export function getTapeEvents(tapeId: string, signal?: AbortSignal) {
  return get<TapeEvents>('/tapes/' + encodeURIComponent(tapeId) + '/events?limit=100', signal)
}

export function getSkillDiff(skillId: string, signal?: AbortSignal) {
  return get<SkillDiff>('/skills/' + encodeURIComponent(skillId) + '/diff', signal)
}

export function getRun(runId: string, signal?: AbortSignal) {
  return get<StoredDocument>('/runs/' + encodeURIComponent(runId), signal)
}

export function getReceipt(receiptId: string, signal?: AbortSignal) {
  return get<StoredDocument>('/receipts/' + encodeURIComponent(receiptId), signal)
}

export function formatEventPayload(event: TapeEvent): string {
  return JSON.stringify(event.payload, null, 2)
}

export function formatDate(timestampMs: number | null): string {
  if (timestampMs === null) return 'Not finished'
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestampMs))
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat().format(value)
}

export function apiBaseLabel(): string {
  return API_BASE
}
