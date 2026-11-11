export interface RunContext {
  schema: 'skilltape.dev/run-context/v1'
  receipt_sha256: string
  run_id: string
  skill_hash: string
  input_set_sha256: string
  input_observation: 'bounded-before-run'
  executor: string
  executor_version: string
  os: string
  arch: string
  max_processes: number
  step_timeout_ms: number
  max_output_bytes: number
}
export interface ContextReview {
  schema: 'skilltape.dev/context-review/v1'
  id: string
  status: 'recorded' | 'unrecorded' | 'invalid'
  context: RunContext | null
  provenance: 'not-authenticated'
}
export function validContextReview(value: unknown, id: string): value is ContextReview {
  if (!value || typeof value !== 'object') return false
  const v = value as ContextReview
  if (v.schema !== 'skilltape.dev/context-review/v1' || v.id !== id || v.provenance !== 'not-authenticated') return false
  if (v.status === 'unrecorded' || v.status === 'invalid') return v.context === null
  const c = v.context
  if (v.status !== 'recorded' || !c || Object.keys(c).length !== 13) return false
  return c.schema === 'skilltape.dev/run-context/v1' && c.input_observation === 'bounded-before-run' && c.executor === 'skilltape-runner/sequential-local-v1' &&
    [c.receipt_sha256, c.run_id, c.skill_hash, c.input_set_sha256].every(v => typeof v === 'string' && /^[a-f0-9]{64}$/.test(v)) &&
    [c.executor_version, c.os, c.arch].every(v => typeof v === 'string' && /^[A-Za-z0-9._-]{1,64}$/.test(v)) &&
    [c.max_processes, c.step_timeout_ms, c.max_output_bytes].every(v => Number.isSafeInteger(v) && v > 0)
}
export function compareRunContexts(before: ContextReview, after: ContextReview): string[] | null {
  if (before.status !== 'recorded' || after.status !== 'recorded' || !before.context || !after.context) return null
  const a = before.context, b = after.context
  const differences: string[] = []
  if (a.input_set_sha256 !== b.input_set_sha256) differences.push('Input snapshot changed')
  if (a.skill_hash !== b.skill_hash) differences.push('Skill package changed')
  if (a.executor !== b.executor || a.executor_version !== b.executor_version) differences.push('Executor changed')
  if (a.os !== b.os || a.arch !== b.arch) differences.push('Platform changed')
  if (a.max_processes !== b.max_processes || a.step_timeout_ms !== b.step_timeout_ms || a.max_output_bytes !== b.max_output_bytes) differences.push('Resource limits changed')
  return differences
}
