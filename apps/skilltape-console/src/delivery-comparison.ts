import type { DeliveryReview, FileSummary } from './types'

export interface DeliveryChange {
  path: string
  kind: 'added' | 'removed' | 'changed'
  before?: FileSummary
  after?: FileSummary
}

function completeFiles(report: DeliveryReview): boolean {
  if (report.status !== 'passed' || report.receipt_binding !== 'verified' ||
    report.files_truncated !== false || report.findings_truncated !== false || report.findings.length !== 0 ||
    report.files.length > 100 || report.declared_files !== report.checked_files ||
    report.checked_files !== report.files.length || report.declared_bytes !== report.checked_bytes) return false
  const paths = new Set<string>()
  let bytes = 0
  for (const file of report.files) {
    if (typeof file.path !== 'string' || !file.path || file.path.length > 1024 ||
      file.path.includes('\\') || file.path.includes('\0') || file.path.split('/').some(part => !part || part === '.' || part === '..') ||
      paths.has(file.path) || !Number.isSafeInteger(file.bytes) || file.bytes < 0 || file.bytes > 16 * 1024 * 1024 ||
      typeof file.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(file.sha256)) return false
    paths.add(file.path)
    bytes += file.bytes
  }
  return bytes <= 64 * 1024 * 1024 && bytes === report.checked_bytes
}

export function compareDeliveries(before: DeliveryReview, after: DeliveryReview) {
  if (!completeFiles(before) || !completeFiles(after)) return null
  const left = new Map(before.files.map(file => [file.path, file]))
  const right = new Map(after.files.map(file => [file.path, file]))
  const counts = { added: 0, removed: 0, changed: 0, unchanged: 0 }
  const changes: DeliveryChange[] = []
  for (const path of [...new Set([...left.keys(), ...right.keys()])].sort()) {
    const a = left.get(path), b = right.get(path)
    const kind = !a ? 'added' : !b ? 'removed' : a.bytes !== b.bytes || a.sha256 !== b.sha256 ? 'changed' : 'unchanged'
    counts[kind]++
    // Each input has at most 100 details; retain at most 100 changed rows.
    if (kind !== 'unchanged' && changes.length < 100) changes.push({ path, kind, before: a, after: b })
  }
  return { counts, changes, truncated: counts.added + counts.removed + counts.changed > changes.length }
}
