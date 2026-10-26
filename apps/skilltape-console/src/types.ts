export type RedactionState = 'unredacted' | 'redacted' | 'partially_redacted'

export interface Collection<T> {
  schema: string
  items: T[]
  offset: number
  limit: number
  total: number
  next_offset: number | null
}

export interface WorkspaceSummary {
  id: string
  name: string
  tape_count: number
  skill_count: number
  run_count: number
  receipt_count: number
}

export interface TapeSummary {
  id: string
  recorded_id?: string
  schema: string
  started_at_ms: number
  finished_at_ms: number | null
  platform: string
  workspace_root: string
  event_count: number
}

export interface TapeEvent {
  sequence: number
  occurred_at_ms: number
  kind: string
  source: string
  payload: Record<string, unknown>
  redaction: RedactionState
}

export interface TapeEvents {
  schema: string
  tape_id: string
  events: TapeEvent[]
  offset: number
  limit: number
  total: number
  next_offset: number | null
}

export interface FileSummary {
  path: string
  bytes: number
  sha256: string
}

export interface DiagnosticSummary {
  code: string
  level: 'error' | 'warning'
  file: string
  path: string
  message: string
}

export interface LintSummary {
  files_checked: number
  errors: DiagnosticSummary[]
  warnings: DiagnosticSummary[]
}

export interface Permissions {
  filesystem: {
    read: string[]
    write: string[]
  }
  process: {
    executables: string[]
    max_processes: number
    default_timeout_ms: number
  }
  network: {
    enabled: boolean
    allow_hosts: string[]
  }
  secrets: {
    read_environment: boolean
  }
}

export interface SkillDiff {
  schema: string
  id: string
  package_path: string
  manifest: Record<string, unknown>
  workflow: {
    schema?: string
    steps?: Array<Record<string, unknown>>
    [key: string]: unknown
  }
  permissions: Permissions
  lockfile: Record<string, unknown>
  files: FileSummary[]
  lint: LintSummary
  policy_plan?: PolicyPlan | null
}

export interface PolicyPlan {
  schema: 'skilltape.dev/policy-plan/v1'
  execution: 'not-run'
  allowed: boolean
  package_valid: boolean
  package_error_codes: string[]
  checks: Array<Omit<PolicyDecision, 'phase'> & { ordinal: number }>
}

export interface StoredDocument {
  schema: string
  id: string
  document: Record<string, unknown>
}

export interface ReceiptStep {
  step_id: string
  status: string
  exit_code: number | null
  stdout_sha256: string
  stdout_bytes: number
  stdout_truncated: boolean
  stderr_sha256: string
  stderr_bytes: number
  stderr_truncated: boolean
}

export interface AssertionResult {
  kind: 'file_exists' | 'file_hash' | 'file_text_contains' | 'command_exit'
  target: string
  passed: boolean
  reason: string
}

export interface PolicyDecision {
  step_id: string
  phase: string
  allowed: boolean
  code: string
  reason: string
  risk: string
}

export interface Receipt {
  schema: string
  run_id: string
  skill_hash: string
  status: 'succeeded' | 'run_failed' | 'cancelled' | 'assertion_failed'
  steps: ReceiptStep[]
  assertions: AssertionResult[]
  policy_decisions: PolicyDecision[]
}

export interface DeliverySummary {
  id: string
  metadata_valid: boolean
  reported_status: string | null
  run_id: string | null
  declared_files: number | null
  declared_bytes: number | null
}

export interface DeliveryReview {
  schema: string
  id: string
  status: 'passed' | 'failed'
  reported_status: string | null
  run_id: string | null
  receipt_binding: 'verified' | 'failed'
  requirement_validation: 'not-run'
  provenance: 'not-authenticated'
  declared_files: number | null
  declared_bytes: number | null
  checked_files: number
  checked_bytes: number
  files: FileSummary[]
  files_truncated: boolean
  findings: Array<{ code: string; path: string | null }>
  findings_truncated: boolean
}

export interface TapeCoverage {
  schema: 'skilltape.dev/tape-coverage/v1'
  tape_id: string
  state: 'complete' | 'open' | 'changed'
  total_events: number
  redaction: Record<RedactionState, number>
  by_kind: Record<string, number>
  by_source: Record<string, number>
  capture_warnings: number
  first_event_at_ms: number | null
  last_event_at_ms: number | null
  samples: Array<{ sequence: number; redaction: RedactionState; capture_warning: boolean }>
  sample_total: number
  samples_truncated: boolean
  limits: { max_events: number; max_bytes: number; sample_limit: number }
}
