import {validReceiptComparison, type ReceiptComparison} from './receipt-comparison'
import {validVerifyAttempt,type VerifyAttempt} from './verify-attempts'
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
    throw new ApiError("连接失败。请启动本机控制台后重试。", 0, 'connection_failed')
  }
  const payload: unknown = await response.json().catch(() => null)
  if (!response.ok) {
    const error =
      payload && typeof payload === 'object' && 'error' in payload
        ? (payload as { error?: { message?: string; code?: string } }).error
        : undefined
    throw new ApiError(
      requestFailure(error?.code, response.status),
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

export async function getSkills(signal?: AbortSignal, query = '', offset = 0): Promise<Collection<string>> {
  const value = await get<unknown>('/skills?limit=50&offset=' + offset + '&q=' + encodeURIComponent(query), signal)
  if (!isCollection(value, 50, (id: unknown): id is string => typeof id === 'string' && isSavedEvidenceId(id)) ||
      value.limit > 50 || value.total > 1000 || value.offset !== offset ||
      new Set(value.items).size !== value.items.length ||
      value.items.length !== Math.min(value.limit, Math.max(0, value.total - offset)) ||
      value.next_offset !== (offset + value.items.length < value.total ? offset + value.items.length : null)) {
    throw new ApiError("技能包列表无效，请重试或输入已知 ID。", 502, 'invalid_skill_list')
  }
  return value
}

export async function getBatchReview(signal?: AbortSignal) {
  const value=await get<unknown>('/batch',signal)
  if(!validBatchReview(value)) throw new ApiError("批次进度无效或版本不受支持，请先检查已保存的工作区。",502,'invalid_batch')
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
    throw new ApiError("已保存技能包无效或版本不受支持，请用当前 CLI 检查后重试。", 502, 'invalid_skill')
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
    throw new ApiError("策略预检无效或版本不受支持，未批准任何执行。", 502, 'invalid_policy_plan')
  }
  return skill
}

export async function getRun(runId: string, signal?: AbortSignal) {
  const value = await get<unknown>('/runs/' + encodeURIComponent(runId), signal)
  if (!isRunDocument(value, runId)) {
    throw new ApiError("已保存运行无效或版本不受支持。请检查原始记录，控制台没有运行或修改它。", 502, 'invalid_run')
  }
  return value
}

export async function getReceipt(receiptId: string, signal?: AbortSignal) {
  const payload = await get<unknown>('/receipts/' + encodeURIComponent(receiptId), signal)
  if (!isRecord(payload) || payload.schema !== 'skilltape.dev/receipt/v1' || payload.id !== receiptId || !isReceipt(payload.document)) {
    throw new ApiError("已保存凭据无效或版本不受支持，请选择另一份凭据或检查文件后重试。", 502, 'invalid_receipt')
  }
  return { ...payload, document: payload.document }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function boundedPayload(value: unknown): boolean {
  let nodes = 0
  function inspect(item: unknown, depth: number): boolean {
    if (++nodes > 100000 || depth > 64) return false
    if (typeof item === 'string') return item.length <= 65536
    if (typeof item === 'number') return Number.isFinite(item) && Math.abs(item) <= Number.MAX_SAFE_INTEGER
    if (Array.isArray(item)) return item.length <= 10000 && item.every(child => inspect(child, depth + 1))
    if (isRecord(item)) {
      const keys = Object.keys(item)
      return keys.length <= 10000 && keys.every(key => key.length <= 65536 && inspect(item[key], depth + 1))
    }
    return item === null || typeof item === 'boolean'
  }
  return inspect(value, 0)
}

function isRunDocument(value: unknown, id: string): value is StoredDocument {
  if (!boundedPayload(value) || !isRecord(value) || value.id !== id || !isRecord(value.document)) return false
  const document = value.document
  const keys = (item: Record<string, unknown>, allowed: string[], required = allowed) =>
    Object.keys(item).every(key => allowed.includes(key)) && required.every(key => Object.hasOwn(item, key))
  const text = (item: unknown): item is string => typeof item === 'string' && item.length > 0 && item.length <= 65536
  const digest = (item: unknown) => typeof item === 'string' && /^[a-f0-9]{64}$/.test(item)
  const count = (item: unknown) => typeof item === 'number' && Number.isSafeInteger(item) && item >= 0
  const step = (item: unknown) => isRecord(item) && keys(item, ['step_id', 'status', 'exit_code', 'stdout_sha256', 'stdout_bytes', 'stdout_truncated', 'stderr_sha256', 'stderr_bytes', 'stderr_truncated']) &&
    text(item.step_id) && text(item.status) && ['succeeded', 'failed', 'denied', 'timed_out', 'cancelled', 'spawn_failed'].includes(item.status) &&
    (item.exit_code === null || Number.isSafeInteger(item.exit_code)) && digest(item.stdout_sha256) && digest(item.stderr_sha256) &&
    count(item.stdout_bytes) && count(item.stderr_bytes) && typeof item.stdout_truncated === 'boolean' && typeof item.stderr_truncated === 'boolean'
  const legacy = value.schema === 'skilltape.dev/run/v1'
  if (document.schema !== value.schema || (!legacy && value.schema !== 'skilltape.dev/replay/v1') ||
    !text(document.status) || !['succeeded', 'failed', 'cancelled'].includes(document.status) || !Array.isArray(document.steps) || !document.steps.every(step)) return false
  if (legacy) {
    return keys(document, ['schema', 'status', 'steps']) &&
      (document.status !== 'succeeded' || document.steps.every(item => item.status === 'succeeded'))
  }
  if (!keys(document, ['schema', 'status', 'steps', 'policy_decisions', 'output_truncated', 'failure'], ['schema', 'status', 'steps', 'policy_decisions', 'output_truncated']) ||
    typeof document.output_truncated !== 'boolean' || !Array.isArray(document.policy_decisions) ||
    !document.policy_decisions.every(item => isRecord(item) && keys(item, ['step_id', 'phase', 'allowed', 'code', 'reason', 'risk']) &&
      text(item.step_id) && text(item.code) && text(item.reason) && text(item.phase) && ['before', 'after'].includes(item.phase) &&
      typeof item.allowed === 'boolean' && text(item.risk) && ['low', 'medium', 'high', 'critical'].includes(item.risk))) return false
  if (document.failure != null && (!isRecord(document.failure) || !keys(document.failure, ['kind', 'step_id', 'status']) ||
    !text(document.failure.kind) || !['cancelled', 'step_failed'].includes(document.failure.kind) || !text(document.failure.step_id) ||
    document.failure.status !== null && typeof document.failure.status !== 'string')) return false
  return document.status !== 'succeeded' || document.failure == null && document.steps.every(item => item.status === 'succeeded') &&
    document.policy_decisions.every(item => item.allowed === true)
}

const isText = (value: unknown): value is string => typeof value === 'string'
const isCount = (value: unknown): value is number => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
const isTimestamp = (value: unknown): value is number => isCount(value) && Number.isFinite(new Date(value).getTime())
const isTextList = (value: unknown) => Array.isArray(value) && value.every(isText)
const invalidTimeline = () => new ApiError("已保存时间线无效或版本不受支持，请检查原始记录后重试。", 502, 'invalid_timeline')

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
  if (!boundedPayload(value) || !isRecord(value) || value.schema !== 'skilltape.dev/console/v1' || !isText(value.id) || !isText(value.package_path) ||
    !isRecord(value.manifest) || !isRecord(value.workflow) || !isRecord(value.permissions) || !isRecord(value.lockfile) || !isRecord(value.lint)) return false
  const permissions = value.permissions, lint = value.lint
  if (!isRecord(permissions.filesystem) || !isRecord(permissions.process) || !isRecord(permissions.network) || !isRecord(permissions.secrets)) return false
  const viewText = (item: unknown): item is string => isText(item) && item.length <= 4096
  const diagnostic = (item: unknown) => isRecord(item) && viewText(item.code) && viewText(item.file) && viewText(item.path) &&
    viewText(item.message) && isText(item.level) && ['error', 'warning'].includes(item.level)
  return Array.isArray(value.workflow.steps) && value.workflow.steps.every(step => isRecord(step) && viewText(step.id) && isText(step.action) &&
      ['program', 'path', 'operation'].every(field => step[field] === undefined || viewText(step[field]))) &&
    Array.isArray(value.files) && value.files.length <= 64 && value.files.every(file => isRecord(file) && viewText(file.path) && isCount(file.bytes) &&
      isText(file.sha256) && /^[0-9a-f]{64}$/.test(file.sha256)) &&
    isCount(lint.files_checked) && Array.isArray(lint.errors) && lint.errors.every(diagnostic) && Array.isArray(lint.warnings) && lint.warnings.every(diagnostic) && lint.errors.length + lint.warnings.length <= 10000 &&
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
    throw new ApiError("交付包响应无效或版本不受支持，请检查本机控制台服务后刷新列表。", 502, 'invalid_response')
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
    throw new ApiError("交付检查无效或版本不受支持，尚未确认文件验收通过。", 502, 'invalid_response')
  }
  return payload
}

export function formatEventPayload(event: TapeEvent): string {
  return JSON.stringify(event.payload, null, 2)
}

export function formatDate(timestampMs: number | null): string {
  if (timestampMs === null) return "尚未结束"
  if (!isTimestamp(timestampMs)) return "时间戳无效"
  return new Intl.DateTimeFormat('zh-CN', {
    hour12: false,
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(timestampMs))
}

export function formatNumber(value: number): string {
  return new Intl.NumberFormat('zh-CN').format(value)
}

export function apiBaseLabel(): string {
  return API_BASE
}

export async function getRunContext(receiptId: string, signal?: AbortSignal) {
  const value = await get<unknown>('/receipts/' + encodeURIComponent(receiptId) + '/context', signal)
  if (!validContextReview(value, receiptId)) throw new ApiError("运行上下文无效或版本不受支持。", 502, 'invalid_context')
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
  if (!isTapeCoverage(value,id)) throw new ApiError("采集完整性数据无效或版本不受支持，请检查已保存记录后重试。",502,'invalid_coverage')
  return value
}


export interface TapePackages extends Collection<{id:string;matches_tape:boolean;source_state:'matched'|'different'|'unrecorded'|'unavailable'}> {tape_id:string}
export async function getTapePackages(tapeId:string,signal?:AbortSignal,offset=0):Promise<TapePackages>{
 const value=await get<unknown>('/tapes/'+encodeURIComponent(tapeId)+'/packages?limit=50&offset='+offset,signal)
 if(!isCollection(value,50,(row:unknown):row is TapePackages['items'][number]=>Boolean(row&&typeof row==='object'&&typeof (row as {id:string}).id==='string'&&isSavedEvidenceId((row as {id:string}).id)&&['matched','different','unrecorded','unavailable'].includes((row as {source_state:string}).source_state)&&typeof (row as {matches_tape:boolean}).matches_tape==='boolean'&&((row as {source_state:string}).source_state==='matched')===(row as {matches_tape:boolean}).matches_tape))||!('tape_id' in value)||value.tape_id!==tapeId)throw new ApiError("技能包查询不完整或版本不受支持，请重试或输入已知技能包 ID。",502,'invalid_packages')
 return value as TapePackages
}

export interface ReceiptSummary {id:string;metadata_state:'available'|'unavailable';status:string|null;skill_hash:string|null;run_id:string|null;bytes:number|null;modified_at_ms:number|null}
export interface ReceiptFilters {status:''|'succeeded'|'run_failed'|'assertion_failed'|'cancelled'|'unavailable';skillHash:string}
export async function getReceipts(signal?:AbortSignal,offset=0,filters?:ReceiptFilters):Promise<Collection<ReceiptSummary>>{
 const query=new URLSearchParams({limit:'50',offset:String(offset)})
 if(filters?.status)query.set('status',filters.status)
 if(filters?.skillHash)query.set('skill_hash',filters.skillHash)
 const value=await get<unknown>('/receipts?'+query,signal)
 if(!isCollection(value,50,(row:unknown):row is ReceiptSummary=>{if(!row||typeof row!=='object')return false;const v=row as ReceiptSummary;return typeof v.id==='string'&&isSavedEvidenceId(v.id)&&['available','unavailable'].includes(v.metadata_state)&&(v.status===null||['succeeded','run_failed','assertion_failed','cancelled'].includes(v.status))&&[v.skill_hash,v.run_id].every(hash=>hash===null||typeof hash==='string'&&/^[a-f0-9]{64}$/.test(hash))&&[v.bytes,v.modified_at_ms].every(n=>n===null||Number.isSafeInteger(n)&&n>=0)&&(v.metadata_state!=='available'||v.status!==null&&v.skill_hash!==null&&v.run_id!==null)}))throw new ApiError("凭据列表无效，请重试或输入已知文件名。",502,'invalid_receipt_list')
 return value
}


export async function getReceiptComparison(before:string,after:string,signal?:AbortSignal):Promise<ReceiptComparison> {
 if(!isSavedEvidenceId(before)||!isSavedEvidenceId(after)||before===after)throw new ApiError("请选择两个不同的已保存凭据文件名，不要填写路径。",400,'invalid_receipt_pair')
 const value=await get<unknown>('/receipt-comparisons?'+new URLSearchParams({before,after}),signal)
 if(!validReceiptComparison(value,before,after))throw new ApiError("已保存凭据的对比结果无效或不完整。",502,'invalid_receipt_comparison')
 return value
}

export async function getVerifyAttempts(signal?:AbortSignal,offset=0):Promise<Collection<VerifyAttempt>>{
 const value=await get<unknown>('/verify-attempts?'+new URLSearchParams({limit:'12',offset:String(offset)}),signal)
 if(!isCollection(value,12,validVerifyAttempt)||value.total>256||value.offset!==offset||new Set(value.items.map(v=>v.id)).size!==value.items.length)throw new ApiError("单次尝试记录无效或不完整，已保存文件未被修改。",502,'invalid_attempts')
 return value
}
export async function getVerifyAttempt(id:string,signal?:AbortSignal):Promise<VerifyAttempt>{
 if(!/^[a-f0-9]{64}$/.test(id))throw new ApiError("请填写已有尝试 ID，不要填写路径。",400,'invalid_attempt_id')
 const value=await get<unknown>('/verify-attempts/'+encodeURIComponent(id),signal)
 if(!validVerifyAttempt(value)||value.id!==id)throw new ApiError("无法核对单次尝试元数据，已保存文件未被修改。",502,'invalid_attempt')
 return value
}

function requestFailure(rawCode: unknown, status: number): string {
  const code = typeof rawCode === 'string' && /^[a-z0-9_-]{1,64}$/i.test(rawCode) ? rawCode : ''
  const messages: Readonly<Record<string, string>> = {
    not_found: '未找到这份已保存记录，请核对工作区与 ID。',
    unsafe_id: '记录 ID 不安全，请使用目录或文件名，不要填写路径。',
    package_discovery_capacity: '已保存目录超过1,000个，请直接输入已知 ID。',
    attempt_discovery_capacity: '尝试记录超过读取上限，请保留原件并按工作区分别查看。',
    inspection_busy: '已有两项检查正在进行，请等待完成后重试。',
    invalid_document: '已保存记录无效或版本不受支持，请核对原始文件。',
    unsafe_path: '该路径不在工作区允许范围内，请选择工作区中的已保存记录。',
    unsupported_delivery_platform: '当前系统无法完成文件完整性检查。',
  }
  if (status === 404) return messages.not_found!
  if (status === 503) return '本机控制台暂不可用，请确认服务已启动，等待已有检查完成后重试。'
  return (Object.hasOwn(messages, code) ? messages[code] : undefined) ?? `本机服务无法完成请求（HTTP ${status}${code ? '，' + code : ''}），请检查原始记录后重试。`
}
