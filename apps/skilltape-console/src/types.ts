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
  type: string
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
