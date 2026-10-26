import { validContextReview } from './run-context'
import { validBatchReview } from './batch'
import type {
  Collection,
  DeliveryReview,
  DeliverySummary,
  Receipt,
  SkillDiff,
  StoredDocument,
  TapeCoverage,
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
  let response: Response
  try {
    response = await fetch(API_BASE + path, { headers: { Accept: 'application/json' }, signal })
  } catch (error) {
    if (signal?.aborted) throw error
    throw new ApiError('Connection failed. Start the local Console, then retry.', 0, 'connection_failed')
  }
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

export async function getWorkspaces(signal?: AbortSignal) {
  const value = await get<unknown>('/workspaces', signal)
  if (!isCollection(value, 100, isWorkspace)) throw invalidTimeline()
  return value
}

export async function getBatchReview(signal?: AbortSignal) {
  const value=await get<unknown>('/batch',signal)
  if(!validBatchReview(value)) throw new ApiError('Batch progress is invalid or unsupported. Inspect the saved workspace before continuing.',502,'invalid_batch')
  return value
}

export async function getTapes(workspaceId: string, signal?: AbortSignal, offset = 0) {
  const value = await get<unknown>(
    '/workspaces/' + encodeURIComponent(workspaceId) + '/tapes?limit=50&offset=' + offset,
    signal,
  )
  if (!isCollection(value, 50, isTape)) throw invalidTimeline()
  return value
}

export async function getTapeEvents(tapeId: string, signal?: AbortSignal, offset = 0) {
  const value = await get<unknown>('/tapes/' + encodeURIComponent(tapeId) + '/events?limit=100&offset=' + offset, signal)
  if (!isTapeEvents(value, tapeId)) throw invalidTimeline()
  return value
}

export function isSavedEvidenceId(id: string): boolean {
  return id.length > 0 && id.length <= 255 && id !== '.' && id !== '..' && !/[/\\:\u0000-\u001f\u007f]/.test(id)
}

export async function getSkillDiff(skillId: string, signal?: AbortSignal) {
  const skill = await get<unknown>('/skills/' + encodeURIComponent(skillId) + '/diff', signal)
  if (!isSkillDiff(skill) || skill.id !== skillId) {
    throw new ApiError('The saved Skill package is invalid or unsupported. Check it with the current CLI, then retry.', 502, 'invalid_skill')
  }
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

const isText = (value: unknown): value is string => typeof value === 'string'
const isCount = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
const isTimestamp = (value: unknown): value is number => isCount(value) && Number.isFinite(new Date(value).getTime())
const isTextList = (value: unknown) => Array.isArray(value) && value.every(isText)
const invalidTimeline = () => new ApiError('The saved timeline is invalid or unsupported. Check the saved Tape, then retry.', 502, 'invalid_timeline')

function isPage(value: Record<string, unknown>) {
  return value.schema === 'skilltape.dev/console/v1' && isCount(value.offset) &&
    isCount(value.limit) && value.limit > 0 && isCount(value.total) &&
    (value.next_offset === null || (isCount(value.next_offset) && value.next_offset > value.offset && value.next_offset <= value.total))
}

function isCollection<T>(value: unknown, maximum: number, check: (item: unknown) => item is T): value is Collection<T> {
  return isRecord(value) && isPage(value) && Array.isArray(value.items) && value.items.length <= maximum && value.items.every(check)
}

function isWorkspace(value: unknown): value is WorkspaceSummary {
  return isRecord(value) && isText(value.id) && isSavedEvidenceId(value.id) && isText(value.name) &&
    [value.tape_count, value.skill_count, value.run_count, value.receipt_count].every(isCount)
}

function isTape(value: unknown): value is TapeSummary {
  return isRecord(value) && isText(value.id) && isSavedEvidenceId(value.id) && isText(value.schema) &&
    (value.recorded_id === undefined || isText(value.recorded_id)) && isTimestamp(value.started_at_ms) &&
    (value.finished_at_ms === null || isTimestamp(value.finished_at_ms)) && isText(value.platform) &&
    isText(value.workspace_root) && isCount(value.event_count)
}

function isTapeEvents(value: unknown, id: string): value is TapeEvents {
  // Match the requested page size, rather than rendering accumulated history.
  return isRecord(value) && isPage(value) && value.tape_id === id && Array.isArray(value.events) && value.events.length <= 100 &&
    value.events.every(event => isRecord(event) && isCount(event.sequence) && isTimestamp(event.occurred_at_ms) &&
      isText(event.kind) && isText(event.source) && Object.hasOwn(event, 'payload') &&
      isText(event.redaction) && ['unredacted', 'redacted', 'partially_redacted'].includes(event.redaction))
}

function isSkillDiff(value: unknown): value is SkillDiff {
  if (!isRecord(value) || value.schema !== 'skilltape.dev/console/v1' || !isText(value.id) || !isText(value.package_path) ||
    !isRecord(value.manifest) || !isRecord(value.workflow) || !isRecord(value.permissions) || !isRecord(value.lockfile) || !isRecord(value.lint)) return false
  const permissions = value.permissions, lint = value.lint
  if (!isRecord(permissions.filesystem) || !isRecord(permissions.process) || !isRecord(permissions.network) || !isRecord(permissions.secrets)) return false
  const diagnostic = (item: unknown) => isRecord(item) && isText(item.code) && isText(item.file) && isText(item.path) &&
    isText(item.message) && isText(item.level) && ['error', 'warning'].includes(item.level)
  return Array.isArray(value.workflow.steps) && value.workflow.steps.every(step => isRecord(step) && isText(step.id) && isText(step.action) &&
      ['program', 'path', 'operation'].every(field => step[field] === undefined || isText(step[field]))) &&
    Array.isArray(value.files) && value.files.every(file => isRecord(file) && isText(file.path) && isCount(file.bytes) &&
      isText(file.sha256) && /^[0-9a-f]{64}$/.test(file.sha256)) &&
    isCount(lint.files_checked) && Array.isArray(lint.errors) && lint.errors.every(diagnostic) && Array.isArray(lint.warnings) && lint.warnings.every(diagnostic) &&
    isTextList(permissions.filesystem.read) && isTextList(permissions.filesystem.write) &&
    isTextList(permissions.process.executables) && isCount(permissions.process.max_processes) && isCount(permissions.process.default_timeout_ms) &&
    typeof permissions.network.enabled === 'boolean' && isTextList(permissions.network.allow_hosts) && typeof permissions.secrets.read_environment === 'boolean'
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
  const payload = await get<unknown>('/workspaces/default/deliveries?limit=50&offset=' + offset, signal)
  if (!isCollection(payload, 50, isDeliverySummary) || payload.total > 1000 || payload.offset !== offset || payload.limit !== 50) {
    throw new ApiError('The saved-delivery response is invalid or unsupported. Check the local Console service, then refresh the list.', 502, 'invalid_response')
  }
  return payload
}

function isDeliverySummary(value: unknown): value is DeliverySummary {
  const text = (item: unknown) => item === null || (isText(item) && item.length <= 4096)
  return isRecord(value) && isText(value.id) && value.id.length > 0 && value.id.length <= 4096 &&
    typeof value.metadata_valid === 'boolean' && text(value.reported_status) && text(value.run_id) &&
    (value.declared_files === null || isCount(value.declared_files)) && (value.declared_bytes === null || isCount(value.declared_bytes))
}

function isDeliveryReview(value: unknown, id: string): value is DeliveryReview {
  const text = (item: unknown) => isText(item) && item.length <= 4096
  const optionalText = (item: unknown) => item === null || text(item)
  return isRecord(value) && value.schema === 'skilltape.dev/delivery-review/v1' && value.id === id &&
    (value.status === 'passed' || value.status === 'failed') && value.requirement_validation === 'not-run' &&
    value.provenance === 'not-authenticated' && (value.receipt_binding === 'verified' || value.receipt_binding === 'failed') &&
    optionalText(value.reported_status) && optionalText(value.run_id) &&
    (value.declared_files === null || isCount(value.declared_files)) && (value.declared_bytes === null || isCount(value.declared_bytes)) &&
    isCount(value.checked_files) && isCount(value.checked_bytes) && typeof value.files_truncated === 'boolean' && typeof value.findings_truncated === 'boolean' &&
    Array.isArray(value.files) && value.files.length <= 100 && value.files.every(file => isRecord(file) && text(file.path) &&
      isCount(file.bytes) && isText(file.sha256) && /^[0-9a-f]{64}$/.test(file.sha256)) &&
    Array.isArray(value.findings) && value.findings.length <= 64 && value.findings.every(finding => isRecord(finding) && text(finding.code) && optionalText(finding.path))
}

export async function getDelivery(id: string, signal?: AbortSignal) {
  const payload = await get<unknown>('/deliveries/' + encodeURIComponent(id), signal)
  if (!isDeliveryReview(payload, id)) {
    throw new ApiError('The saved-delivery inspection is invalid or unsupported. No file acceptance is established.', 502, 'invalid_response')
  }
  return payload
}

export function formatEventPayload(event: TapeEvent): string {
  return JSON.stringify(event.payload, null, 2)
}

export function formatDate(timestampMs: number | null): string {
  if (timestampMs === null) return 'Not finished'
  if (!isTimestamp(timestampMs)) return 'Invalid timestamp'
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

export async function getRunContext(receiptId: string, signal?: AbortSignal) {
  const value = await get<unknown>('/receipts/' + encodeURIComponent(receiptId) + '/context', signal)
  if (!validContextReview(value, receiptId)) throw new ApiError('Run context is invalid or unsupported.', 502, 'invalid_context')
  return value
}

const coverageKinds = ['session_started','session_finished','terminal_command','filesystem_changed','permission_requested','permission_decided','environment_snapshot','capture_warning']
const coverageSources = ['cli','shell','filesystem','permission','environment','system']
function isTapeCoverage(value: unknown, id: string): value is TapeCoverage {
  const counts = (item: unknown, allowed: string[]): item is Record<string,number> => isRecord(item) && Object.keys(item).length <= allowed.length && Object.entries(item).every(([key,count]) => allowed.includes(key) && isCount(count))
  if (!isRecord(value) || value.schema !== 'skilltape.dev/tape-coverage/v1' || value.tape_id !== id ||
    !isText(value.state) || !['complete','open','changed'].includes(value.state) || !isCount(value.total_events) || value.total_events > 100000 ||
    !counts(value.redaction,['redacted','unredacted','partially_redacted']) || Object.keys(value.redaction).length !== 3 ||
    !counts(value.by_kind,coverageKinds) || !counts(value.by_source,coverageSources) || !isCount(value.capture_warnings) ||
    !isCount(value.sample_total) || value.sample_total > value.total_events || typeof value.samples_truncated !== 'boolean' ||
    !isRecord(value.limits) || value.limits.max_events !== 100000 || value.limits.max_bytes !== 16777216 || value.limits.sample_limit !== 32 ||
    !Array.isArray(value.samples) || value.samples.length !== Math.min(32,value.sample_total) || value.samples_truncated !== (value.sample_total > value.samples.length)) return false
  const sum = (item: Record<string,unknown>) => Object.values(item).reduce<number>((total,count) => total + Number(count),0)
  if (sum(value.redaction) !== value.total_events || sum(value.by_kind) !== value.total_events || sum(value.by_source) !== value.total_events ||
      value.capture_warnings !== (value.by_kind.capture_warning ?? 0) ||
      value.sample_total < Number(value.redaction.unredacted) + Number(value.redaction.partially_redacted) || value.sample_total < value.capture_warnings || value.sample_total > Math.min(value.total_events,Number(value.redaction.unredacted)+Number(value.redaction.partially_redacted)+value.capture_warnings)) return false
  if (value.total_events === 0 ? value.first_event_at_ms !== null || value.last_event_at_ms !== null : !isTimestamp(value.first_event_at_ms) || !isTimestamp(value.last_event_at_ms)) return false
  const total=value.total_events
  return value.samples.every((sample,index,items) => isRecord(sample) && isCount(sample.sequence) && sample.sequence < total &&
    (index === 0 || sample.sequence > (items[index-1] as {sequence:number}).sequence) &&
    isText(sample.redaction) && ['redacted','unredacted','partially_redacted'].includes(sample.redaction) && typeof sample.capture_warning === 'boolean' &&
    (sample.redaction !== 'redacted' || sample.capture_warning))
}
export async function getTapeCoverage(id: string, signal?: AbortSignal) {
  const value = await get<unknown>('/tapes/' + encodeURIComponent(id) + '/coverage',signal)
  if (!isTapeCoverage(value,id)) throw new ApiError('Capture coverage is invalid or unsupported. Check the saved Tape, then retry.',502,'invalid_coverage')
  return value
}
