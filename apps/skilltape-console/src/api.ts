import type {
  Collection,
  DeliveryReview,
  DeliverySummary,
  Receipt,
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

export async function getSkillDiff(skillId: string, signal?: AbortSignal) {
  const skill = await get<SkillDiff>('/skills/' + encodeURIComponent(skillId) + '/diff', signal)
  const plan = skill.policy_plan
  if (plan != null && (plan.schema !== 'skilltape.dev/policy-plan/v1' || plan.execution !== 'not-run' ||
    typeof plan.allowed !== 'boolean' || typeof plan.package_valid !== 'boolean' ||
    !Array.isArray(plan.package_error_codes) || plan.package_error_codes.length > 64 ||
    !plan.package_error_codes.every(code => typeof code === 'string' && /^[A-Z0-9_]{1,64}$/.test(code)) ||
    !Array.isArray(plan.checks) || plan.checks.length > 1000 ||
    !plan.checks.every((check, index) => check && check.ordinal === index + 1 && typeof check.step_id === 'string' && check.step_id.length <= 4096 &&
      typeof check.allowed === 'boolean' && typeof check.code === 'string' && /^[A-Z0-9_]{1,64}$/.test(check.code) &&
      typeof check.reason === 'string' && check.reason.length <= 512 && ['low', 'medium', 'high', 'critical'].includes(check.risk)) ||
    (plan.allowed && (!plan.package_valid || plan.package_error_codes.length > 0 || plan.checks.some(check => !check.allowed))))) {
    throw new ApiError('The policy preflight is invalid or unsupported. No execution is approved.', 502, 'invalid_policy_plan')
  }
  return skill
}

export function getRun(runId: string, signal?: AbortSignal) {
  return get<StoredDocument>('/runs/' + encodeURIComponent(runId), signal)
}

export async function getReceipt(receiptId: string, signal?: AbortSignal) {
  const payload = await get<unknown>('/receipts/' + encodeURIComponent(receiptId), signal)
  if (!isRecord(payload) || payload.schema !== 'skilltape.dev/receipt/v1' || payload.id !== receiptId || !isReceipt(payload.document)) {
    throw new ApiError('The saved Receipt is invalid or unsupported. Choose another saved Receipt or retry after checking the file.', 502, 'invalid_receipt')
  }
  return { ...payload, document: payload.document }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function isReceipt(value: unknown): value is Receipt {
  const text = (item: unknown): item is string => typeof item === 'string' && item.length > 0 && item.length <= 4096
  const digest = (item: unknown) => typeof item === 'string' && /^[0-9a-f]{64}$/.test(item)
  const count = (item: unknown) => typeof item === 'number' && Number.isSafeInteger(item) && item >= 0
  const keys = (item: Record<string, unknown>, allowed: string[]) =>
    Object.keys(item).length === allowed.length && allowed.every(key => Object.hasOwn(item, key))
  // Bound rendered rows and text; the local API's file-read budget is separate.
  const list = (items: unknown, check: (item: Record<string, unknown>) => boolean) =>
    Array.isArray(items) && items.length <= 1000 && items.every(item => isRecord(item) && check(item))
  if (!isRecord(value) || !keys(value, ['schema', 'run_id', 'skill_hash', 'status', 'steps', 'assertions', 'policy_decisions']) ||
    value.schema !== 'skilltape.dev/receipt/v1' || !digest(value.run_id) || !digest(value.skill_hash) ||
    !text(value.status) || !['succeeded', 'run_failed', 'cancelled', 'assertion_failed'].includes(value.status) ||
    !list(value.steps, item => keys(item, ['step_id', 'status', 'exit_code', 'stdout_sha256', 'stdout_bytes', 'stdout_truncated', 'stderr_sha256', 'stderr_bytes', 'stderr_truncated']) && text(item.step_id) && text(item.status) &&
      ['succeeded', 'failed', 'denied', 'timed_out', 'cancelled', 'spawn_failed'].includes(item.status) &&
      (item.exit_code === null || Number.isSafeInteger(item.exit_code)) &&
      digest(item.stdout_sha256) && digest(item.stderr_sha256) && count(item.stdout_bytes) && count(item.stderr_bytes) &&
      typeof item.stdout_truncated === 'boolean' && typeof item.stderr_truncated === 'boolean') ||
    !list(value.assertions, item => keys(item, ['kind', 'target', 'passed', 'reason']) && text(item.kind) && ['file_exists', 'file_hash', 'file_text_contains', 'command_exit'].includes(item.kind) &&
      text(item.target) && text(item.reason) && typeof item.passed === 'boolean') ||
    !list(value.policy_decisions, item => keys(item, ['step_id', 'phase', 'allowed', 'code', 'reason', 'risk']) && text(item.step_id) && text(item.phase) && ['before', 'after'].includes(item.phase) &&
      typeof item.allowed === 'boolean' && text(item.code) && text(item.reason) && text(item.risk) &&
      ['low', 'medium', 'high', 'critical'].includes(item.risk))) return false
  return value.status !== 'succeeded' || !(value.assertions as Receipt['assertions']).some(item => !item.passed)
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
