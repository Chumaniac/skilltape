import type {
  Collection,
  DeliveryReview,
  DeliverySummary,
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

export function getTapes(workspaceId: string, signal?: AbortSignal, offset = 0) {
  return get<Collection<TapeSummary>>(
    '/workspaces/' + encodeURIComponent(workspaceId) + '/tapes?limit=50&offset=' + offset,
    signal,
  )
}

export function getTapeEvents(tapeId: string, signal?: AbortSignal, offset = 0) {
  return get<TapeEvents>('/tapes/' + encodeURIComponent(tapeId) + '/events?limit=100&offset=' + offset, signal)
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

export async function getDeliveries(offset = 0, signal?: AbortSignal) {
  const payload = await get<Collection<DeliverySummary>>('/workspaces/default/deliveries?limit=50&offset=' + offset, signal)
  if (!payload || !Array.isArray(payload.items) || payload.items.length > 50 ||
    !Number.isSafeInteger(payload.total) || payload.total < 0 || payload.total > 1000 ||
    !payload.items.every(item => item && typeof item.id === 'string' && typeof item.metadata_valid === 'boolean')) {
    throw new ApiError('The saved-delivery response is invalid or unsupported. Use the current source API.', 502, 'invalid_response')
  }
  return payload
}

export async function getDelivery(id: string, signal?: AbortSignal) {
  const payload = await get<DeliveryReview>('/deliveries/' + encodeURIComponent(id), signal)
  if (!payload || payload.schema !== 'skilltape.dev/delivery-review/v1' || payload.id !== id ||
    !['passed', 'failed'].includes(payload.status) || payload.requirement_validation !== 'not-run' ||
    payload.provenance !== 'not-authenticated' || !Array.isArray(payload.files) || payload.files.length > 100 ||
    !Array.isArray(payload.findings) || payload.findings.length > 64) {
    throw new ApiError('The saved-delivery inspection is invalid or unsupported. No file acceptance is established.', 502, 'invalid_response')
  }
  return payload
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
