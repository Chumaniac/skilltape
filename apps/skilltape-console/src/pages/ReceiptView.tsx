import {ReceiptLibrary} from './ReceiptLibrary'
import { RunConditions } from './RunConditions'

import { useEffect, useRef, useState } from 'react'

import { formatNumber, getReceipt, isSavedEvidenceId } from '../api'
import type { Receipt } from '../types'
import { CodeBlock, EmptyState, JsonDisclosure, PageError, PageLoading, StatusBadge, evidenceLabel } from '../ui'

interface ReceiptViewProps {
  receiptId?: string
  skillId?: string
}

function ReceiptPicker({ receiptId, onOpen }: { receiptId: string; onOpen: (id: string) => void }) {
  const [error, setError] = useState('')
  return (
    <form className="panel receipt-picker" onSubmit={(event) => {
      event.preventDefault()
      const id = String(new FormData(event.currentTarget).get('receipt-id') ?? '').trim().replace(/\.json$/i, '')
      if (!isSavedEvidenceId(id)) {
        setError("请填写已保存文件名，不要填写路径。")
        return
      }
      setError('')
      onOpen(id)
    }}>
      <label htmlFor="receipt-id">已保存凭据 ID</label>
      <div className="receipt-picker-controls">
        <input id="receipt-id" name="receipt-id" defaultValue={receiptId} maxLength={255} required
          aria-describedby="receipt-picker-help receipt-picker-error" aria-invalid={error ? true : undefined}
          onInput={() => setError('')} autoComplete="off" spellCheck={false} />
        <button className="button" type="submit">打开凭据</button>
      </div>
      <p id="receipt-picker-help" className="muted">请填写 .skilltape/receipts 中的文件名，可省略 .json。</p>
      <p id="receipt-picker-error" role="alert">{error}</p>
    </form>
  )
}

export function ReceiptViewPage({ receiptId = '', skillId = '' }: ReceiptViewProps) {
  const [receipt, setReceipt] = useState<Receipt | null>(null)
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(true)
  const [reload, setReload] = useState(0)
  const [failedOnly, setFailedOnly] = useState(false)
  const resultHeading = useRef<HTMLHeadingElement>(null)

  useEffect(() => {
    if (!receiptId) { setLoading(false); return }
    if (!isSavedEvidenceId(receiptId)) {
      setError("请填写已保存文件名，不要填写路径。")
      setLoading(false)
      return
    }
    const controller = new AbortController()
    setLoading(true)
    setError('')
    setFailedOnly(false)
    getReceipt(receiptId, controller.signal)
      .then((stored) => { if (!controller.signal.aborted) setReceipt(stored.document) })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setError(reason instanceof Error ? reason.message : "无法读取验证凭据。")
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [receiptId, reload])

  useEffect(() => {
    if (loading || error || !receipt) return
    resultHeading.current?.focus({ preventScroll: true })
    resultHeading.current?.scrollIntoView({ block: 'start', behavior: 'instant' })
  }, [receipt, loading, error])

  const picker = <><ReceiptLibrary onRead={id=>{window.location.hash='receipt?run='+encodeURIComponent(id)+(skillId?'&skill='+encodeURIComponent(skillId):'');if(id===receiptId)setReload(value=>value+1)}}/><ReceiptPicker key={receiptId} receiptId={receiptId} onOpen={(id) => {
    if (id === receiptId) setReload(value => value + 1)
    else window.location.hash = 'receipt?run=' + encodeURIComponent(id) + (skillId ? '&skill=' + encodeURIComponent(skillId) : '')
  }} />{isSavedEvidenceId(skillId) ? <nav className="journey-actions" aria-label="凭据下一步"><a className="button" href={'#compile?skill='+encodeURIComponent(skillId)}>返回技能包复核</a><a className="button" href={'#export?skill='+encodeURIComponent(skillId)+'&run='+encodeURIComponent(receiptId)}>准备导出此技能包</a></nav> : null}</>
  if (loading) return <>{picker}<PageLoading label="正在载入验证凭据…" /></>
  if (error) return <>{picker}<PageError message={error} onRetry={() => setReload((value) => value + 1)} /></>
  if (!receipt) {
    return <>{picker}<EmptyState title="尚未选择凭据" message="运行 skilltape verify，为此页面生成验证凭据。" /></>
  }

  const failed = receipt.assertions.filter(assertion => !assertion.passed)
  const visible = failedOnly ? failed : receipt.assertions
  const statusTone = receipt.status === 'succeeded' ? 'good' : receipt.status === 'assertion_failed' ? 'warn' : 'danger'
  const reviewChecks = failed.length > 0 || (receipt.status === 'succeeded' && receipt.assertions.length > 0)
  const nextStep = failed.length > 0
    ? { title: `${formatNumber(failed.length)} ${failed.length === 1 ? "项检查需要处理" : "项检查需要处理"}`,
      detail: "再次验证前，请将失败目标与预期输出逐项核对。", action: "复核失败检查" }
    : receipt.status === 'succeeded' && receipt.assertions.length > 0
      ? { title: "本次运行已完成", detail: "复核此技能包版本已保存的检查与权限。", action: "复核已保存检查" }
      : receipt.status === 'succeeded'
        ? { title: "运行已完成，但未设置显式检查", detail: "再次使用此技能包前，请复核执行结果与策略决策。", action: "复核步骤结果" }
      : { title: "复核执行结果", detail: "修改技能包前，请阅读步骤结果与策略决策。", action: "复核步骤结果" }
  function openReview() {
    setFailedOnly(failed.length > 0)
    const heading = document.getElementById(reviewChecks ? 'receipt-checks-heading' : 'receipt-steps-heading')
    heading?.focus({ preventScroll: true })
    heading?.scrollIntoView({ block: 'start' })
  }
  return (
    <>{picker}<section className="page-section receipt-review">
      <header className="page-heading">
        <div>
          <p className="eyebrow">已保存运行</p>
          <h1 id="receipt-result-title" ref={resultHeading} tabIndex={-1}>验证凭据</h1>
          <p className="page-description">
            本次运行记录的步骤结果、输出检查与策略决策。
          </p>
        </div>
        <StatusBadge tone={statusTone}>{evidenceLabel(receipt.status)}</StatusBadge>
      </header>

      <section className="receipt-verdict" aria-label="下一步需要复核什么">
        <div><h2>{nextStep.title}</h2><p>{nextStep.detail}</p></div>
        <button className="button" type="button" onClick={openReview}>{nextStep.action}</button>
      </section>
      <div className="receipt-run-details">
        <p className="receipt-run-id"><span>运行 ID</span><code>{receipt.run_id}</code></p>
        <dl className="receipt-facts">
          <div><dt>步骤</dt><dd>{formatNumber(receipt.steps.length)}</dd></div>
          <div><dt>检查项</dt><dd>{formatNumber(receipt.assertions.length)}</dd></div>
          <div><dt>策略决策</dt><dd>{formatNumber(receipt.policy_decisions.length)}</dd></div>
        </dl>
      </div>

      <RunConditions receiptId={receiptId} />
      <div className="review-grid">
        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">执行轨迹</p>
              <h2 id="receipt-steps-heading" tabIndex={-1}>步骤结果</h2>
            </div>
            <span className="panel-note">输出摘要与字节数</span>
          </div>
          {receipt.steps.length > 0 ? (
            <div className="table-wrap" role="region" aria-labelledby="receipt-steps-heading" tabIndex={0}>
              <table>
                <thead>
                  <tr>
                    <th scope="col">步骤</th>
                    <th scope="col">状态</th>
                    <th scope="col">退出</th>
                    <th scope="col">标准输出</th>
                    <th scope="col">标准错误输出</th>
                  </tr>
                </thead>
                <tbody>
                  {receipt.steps.map((step) => (
                    <tr key={step.step_id}>
                      <th scope="row"><code>{step.step_id}</code></th>
                      <td><StatusBadge tone={step.status === 'succeeded' ? 'good' : 'danger'}>{evidenceLabel(step.status)}</StatusBadge></td>
                      <td>{step.exit_code === null ? '—' : step.exit_code}</td>
                      <td><code>{step.stdout_sha256.slice(0, 10)}…</code><span className="table-sub">{formatNumber(step.stdout_bytes)} 字节</span></td>
                      <td><code>{step.stderr_sha256.slice(0, 10)}…</code><span className="table-sub">{formatNumber(step.stderr_bytes)} 字节</span></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <EmptyState title="没有步骤摘要" message="本次运行没有保存步骤级证据。" />
          )}
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">断言</p>
              <h2 id="receipt-checks-heading" tabIndex={-1}>预期结果检查</h2>
            </div>
          </div>
          <p role="status">{receipt.assertions.length - failed.length} 通过 · {failed.length} 失败</p>
          <div className="receipt-filters" role="group" aria-label="断言筛选">
            <button className="button button-secondary" type="button" aria-pressed={!failedOnly} onClick={() => setFailedOnly(false)}>全部检查（{receipt.assertions.length})</button>
            <button className="button button-secondary" type="button" aria-pressed={failedOnly} onClick={() => setFailedOnly(true)}>待处理（{failed.length})</button>
          </div>
          {receipt.assertions.length > 0 ? (
            visible.length > 0 ? <ul className="result-list">
              {visible.map((assertion, index) => (
                <li key={assertion.target + index}>
                  <span className={assertion.passed ? 'result-icon result-pass' : 'result-icon result-fail'} aria-hidden="true">
                    {assertion.passed ? '✓' : '!'}
                  </span>
                  <div>
                    <strong>{evidenceLabel(assertion.kind)}</strong>
                    <StatusBadge tone={assertion.passed ? 'good' : 'danger'}>{assertion.passed ? "通过" : "失败"}</StatusBadge>
                    <span>{assertion.target}</span>
                    <small>{assertion.reason}</small>
                  </div>
                </li>
              ))}
            </ul> : <p className="muted">没有失败断言，请另行复核运行状态和策略决策。</p>
          ) : (
            <p className="muted">未记录显式断言。</p>
          )}
        </article>

        <article className="panel">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">策略</p>
              <h2>决策</h2>
            </div>
          </div>
          {receipt.policy_decisions.length > 0 ? (
            <ul className="decision-list">
              {receipt.policy_decisions.map((decision, index) => (
                <li key={decision.step_id + decision.phase + index}>
                  <StatusBadge tone={decision.allowed ? 'good' : 'danger'}>{decision.allowed ? 'allowed' : 'denied'}</StatusBadge>
                  <div>
                    <strong>{decision.step_id}</strong>
                    <span>{decision.code} · {decision.risk}</span>
                    <small>{decision.reason}</small>
                  </div>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">未记录策略决策。</p>
          )}
        </article>

        <article className="panel panel-span-2">
          <div className="panel-heading">
            <div>
              <p className="eyebrow">完整性</p>
              <h2>技能包哈希</h2>
            </div>
            <span className="panel-note">SHA-256</span>
          </div>
          <CodeBlock value={receipt.skill_hash} label="skill_hash" />
          <JsonDisclosure label="查看凭据 JSON" value={receipt} />
        </article>
      </div>
    </section></>
  )
}
