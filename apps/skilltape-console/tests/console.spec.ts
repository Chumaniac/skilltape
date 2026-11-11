import { expect, test } from '@playwright/test'

const permissions = {
  filesystem: { read: ['inputs/**'], write: ['outputs/**'] },
  process: { executables: ['/usr/bin/printf'], max_processes: 1, default_timeout_ms: 1000 },
  network: { enabled: false, allow_hosts: [] },
  secrets: { read_environment: false },
}

const skill = {
  schema: 'skilltape.dev/console/v1',
  id: 'demo',
  package_path: 'skills/demo',
  manifest: { name: 'demo', version: '0.1.0', targets: ['generic-agent-skill'] },
  workflow: {
    schema: 'skilltape.dev/workflow/v1',
    steps: [
      { action: 'exec', id: 'format', program: '/usr/bin/printf', args: ['ready'] },
      { action: 'file', id: 'copy', operation: 'copy', from: 'inputs/a', to: 'outputs/a' },
    ],
  },
  permissions,
  lockfile: { schema: 'skilltape.dev/lock/v1', tools: [] },
  files: [
    { path: 'skilltape.yaml', bytes: 220, sha256: 'a'.repeat(64) },
    { path: 'workflow.yaml', bytes: 320, sha256: 'b'.repeat(64) },
    { path: 'permissions.json', bytes: 280, sha256: 'c'.repeat(64) },
    { path: 'skilltape.lock', bytes: 100, sha256: 'd'.repeat(64) },
    { path: 'SKILL.md', bytes: 600, sha256: 'e'.repeat(64) },
    { path: 'README.md', bytes: 180, sha256: 'f'.repeat(64) },
  ],
  lint: { files_checked: 6, errors: [], warnings: [] },
}

const receipt = {
  schema: 'skilltape.dev/receipt/v1',
  run_id: 'd'.repeat(64),
  skill_hash: 'a'.repeat(64),
  status: 'succeeded',
  steps: [
    {
      step_id: 'format',
      status: 'succeeded',
      exit_code: 0,
      stdout_sha256: 'b'.repeat(64),
      stdout_bytes: 5,
      stdout_truncated: false,
      stderr_sha256: 'c'.repeat(64),
      stderr_bytes: 0,
      stderr_truncated: false,
    },
  ],
  assertions: [{ kind: 'file_exists', target: 'outputs/a', passed: true, reason: 'file exists' }],
  policy_decisions: [
    { step_id: 'format', phase: 'before', allowed: true, code: 'POLICY001', reason: 'allowed', risk: 'low' },
  ],
}

test('bounded saved run documents preserve explicit legacy and replay identities',async({page})=>{
 await mockApi(page)
 let payload:unknown={schema:'skilltape.dev/run/v1',id:'run-a',document:{status:'succeeded'}}
 await page.route('**/api/v1/runs/run-a',route=>route.fulfill({json:payload}))
 await page.route('**/contract-check',route=>route.fulfill({contentType:'text/html',body:'<!doctype html><title>Isolated API contract</title>'}))
 await page.goto('/contract-check')
 const inspect=()=>page.evaluate(async()=>{
  const module=await import('/src/api.ts')
  try{return {valid:true,schema:(await module.getRun('run-a')).schema}}catch(error){return {valid:false,code:(error as {code?:string}).code}}
 })
 expect(await inspect()).toEqual({valid:false,code:'invalid_run'})
 payload={schema:'skilltape.dev/run/v1',id:'run-a',document:{schema:'skilltape.dev/run/v1',status:'succeeded',steps:[]}}
 expect(await inspect()).toEqual({valid:true,schema:'skilltape.dev/run/v1'})
 payload={schema:'skilltape.dev/replay/v1',id:'run-a',document:{schema:'skilltape.dev/replay/v1',status:'succeeded',steps:[],policy_decisions:[],output_truncated:false}}
 expect(await inspect()).toEqual({valid:true,schema:'skilltape.dev/replay/v1'})
 payload={schema:'skilltape.dev/replay/v1',id:'run-a',document:{schema:'skilltape.dev/replay/v1',status:'succeeded',
  steps:[{...receipt.steps[0],status:{toString:'PRIVATE_RUN_SENTINEL'}}],policy_decisions:[],output_truncated:false}}
 expect(await inspect()).toEqual({valid:false,code:'invalid_run'})
})

test('large compile arrays use bounded workflow and diagnostic pages',async({page})=>{
 await mockApi(page)
 const payload={...skill,workflow:{...skill.workflow,steps:Array.from({length:205},(_,index)=>({id:'step-'+index,action:'exec',program:'printf',args:['x'.repeat(400)]}))},
  lint:{files_checked:6,errors:[],warnings:Array.from({length:205},(_,index)=>({code:'SYNTHETIC'+index,level:'warning',file:'workflow.yaml',path:'/steps/'+index,message:'Synthetic review note'}))}}
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:payload}))
 await page.goto('/#compile?skill=demo')
 await expect(page.locator('.workflow-list > li')).toHaveCount(100)
 await expect(page.locator('.diagnostic-list > li')).toHaveCount(100)
 const json=page.locator('details').filter({has:page.getByText("查看工作流 JSON",{exact:true})})
 await expect(json.locator('pre')).toHaveCount(0)
 await page.getByText("查看工作流 JSON",{exact:true}).click()
 await expect(json.locator('pre')).toContainText('预览已截断')
 expect((await json.locator('pre').innerText()).length).toBeLessThan(65700)
 await page.getByRole('button',{name:"下一页工作流"}).click()
 await expect(page.getByText('step-100',{exact:true})).toBeVisible()
 await page.getByRole('button',{name:"下一页工作流"}).click()
 await expect(page.locator('.workflow-list > li')).toHaveCount(5)
 await page.getByRole('button',{name:"下一页诊断"}).click()
 await expect(page.getByText('SYNTHETIC100',{exact:true})).toBeVisible()
})

test('oversized compile arrays fail explicitly without rendering an unbounded list',async({page})=>{
 await mockApi(page)
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:{...skill,workflow:{...skill.workflow,
  steps:Array.from({length:10001},(_,index)=>({id:'step-'+index,action:'exec',program:'printf'}))}}}))
 await page.goto('/#compile?skill=demo')
 await expect(page.getByRole('alert')).toContainText("已保存技能包无效或版本不受支持")
 await expect(page.locator('.workflow-list > li')).toHaveCount(0)
})

async function mockApi(page: import('@playwright/test').Page, mode: 'normal' | 'error' = 'normal', storedReceipt = receipt) {
  await page.route('**/api/v1/**', async (route) => {
    const url = new URL(route.request().url())
    if (mode === 'error' && url.pathname.endsWith('/workspaces')) {
      await route.fulfill({
        status: 503,
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/api-error/v1',
          error: { code: 'offline', message: '本机控制台暂不可用，请启动服务后重试。' },
        }),
      })
      return
    }
    if (url.pathname.endsWith('/workspaces')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          items: [{ id: 'default', name: 'fixture-workspace', tape_count: 1, skill_count: 1, run_count: 1, receipt_count: 1 }],
          offset: 0,
          limit: 1,
          total: 1,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/workspaces/default/tapes')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          items: [
            {
              id: 'tape-a',
              schema: 'skilltape.dev/tape/v1',
              started_at_ms: 1710000000000,
              finished_at_ms: 1710000001000,
              platform: 'test',
              workspace_root: 'workspace',
              event_count: 1,
            },
          ],
          offset: 0,
          limit: 50,
          total: 1,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/tapes/none/events')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          tape_id: 'none',
          events: [],
          offset: 0,
          limit: 100,
          total: 0,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/tapes/tape-a/events')) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          schema: 'skilltape.dev/console/v1',
          tape_id: 'tape-a',
          events: [
            {
              sequence: 0,
              occurred_at_ms: 1710000000500,
              kind: 'terminal_command',
              source: 'shell',
              payload: { command: 'printf', note: 'redacted timeline payload' },
              redaction: 'redacted',
            },
          ],
          offset: 0,
          limit: 100,
          total: 1,
          next_offset: null,
        }),
      })
      return
    }
    if (url.pathname.endsWith('/skills/demo/diff')) {
      await route.fulfill({ contentType: 'application/json', body: JSON.stringify(skill) })
      return
    }
    if (url.pathname.endsWith('/context')) {
      await route.fulfill({json:{schema:'skilltape.dev/context-review/v1',id:url.pathname.split('/').at(-2),status:'unrecorded',context:null,provenance:'not-authenticated'}})
      return
    }
    if (url.pathname.includes('/receipts/')) {
      if (!url.pathname.endsWith('/receipts/run-a')) {
        await route.fulfill({ status: 404, contentType: 'application/json', body: JSON.stringify({ error: { message: 'Saved Receipt not found.' } }) })
        return
      }
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ schema: 'skilltape.dev/receipt/v1', id: 'run-a', document: storedReceipt }),
      })
      return
    }
    await route.continue()
  })
}

test('timeline displays redaction state and payload details', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#timeline')

  await expect(page.getByRole('heading', { name: "采集时间线" })).toBeVisible()
  await expect(page.getByText("已脱敏", { exact: true })).toBeVisible()
  await page.getByText("查看事件内容").click()
  await expect(page.getByText('redacted timeline payload')).toBeVisible()
  await expect(page.getByText('fixture-workspace')).toBeVisible()
})

test('timeline pages through tapes and events without accumulating earlier pages', async ({ page }, testInfo) => {
  await mockApi(page)
  let failNextPage = true
  await page.route('**/api/v1/workspaces/default/tapes*', async (route) => {
    const offset = Number(new URL(route.request().url()).searchParams.get('offset') ?? 0)
    const tapes = Array.from({ length: 51 }, (_, index) => ({
      id: 'tape-' + index,
      schema: 'skilltape.dev/tape/v1',
      started_at_ms: 0,
      finished_at_ms: 0,
      platform: 'test',
      workspace_root: 'workspace',
      event_count: 101,
    }))
    await route.fulfill({ json: {
      schema: 'skilltape.dev/console/v1', items: tapes.slice(offset, offset + 50),
      offset, limit: 50, total: tapes.length, next_offset: offset === 0 ? 50 : null,
    } })
  })
  await page.route('**/api/v1/tapes/tape-*/events*', async (route) => {
    const url = new URL(route.request().url())
    const offset = Number(url.searchParams.get('offset') ?? 0)
    if (offset === 100 && failNextPage) {
      failNextPage = false
      await route.fulfill({ status: 503, json: { error: { message: '本机控制台暂不可用' } } })
      return
    }
    const events = Array.from({ length: 101 }, (_, sequence) => ({
      sequence, occurred_at_ms: 0, kind: 'terminal_command', source: 'shell',
      payload: { note: 'synthetic event ' + sequence }, redaction: 'redacted',
    }))
    await route.fulfill({ json: {
      schema: 'skilltape.dev/console/v1', tape_id: url.pathname.split('/')[4],
      events: events.slice(offset, offset + 100), offset, limit: 100, total: events.length,
      next_offset: offset === 0 ? 100 : null,
    } })
  })

  await page.goto('/#timeline')
  await expect(page.locator('.event-row')).toHaveCount(100)
  await expect(page.getByRole('button', { name: "上一页事件" })).toBeDisabled()
  await expect(page.getByText("已结束", { exact: true })).toBeVisible()
  await page.getByRole('button', { name: "下一页事件" }).click()
  await expect(page.getByRole('alert')).toContainText('本机控制台暂不可用')
  await page.getByRole('button', { name: "重试" }).click()
  await expect(page.locator('.event-row')).toHaveCount(1)
  await expect(page.getByText('#0100', { exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: "下一页事件" })).toBeDisabled()
  await page.getByRole('button', { name: "上一页事件" }).click()
  await expect(page.locator('.event-row')).toHaveCount(100)
  await expect(page.getByText('#0000', { exact: true })).toBeVisible()

  await page.getByRole('button', { name: "下一页采集记录" }).click()
  await expect(page.getByRole('combobox', { name: "选择采集记录" })).toHaveValue('tape-50')
  await expect(page.locator('select option')).toHaveCount(1)
  await expect(page.locator('.event-row')).toHaveCount(100)
  await page.getByRole('button', { name: "下一页事件" }).click()
  await expect(page.getByText('#0100', { exact: true })).toBeVisible()
  await page.reload()
  await expect(page.getByRole('combobox', { name: "选择采集记录" })).toHaveValue('tape-50')
  await expect(page.getByText('#0100', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: "上一页采集记录" }).click()
  await expect(page.getByRole('combobox', { name: "选择采集记录" })).toHaveValue('tape-0')
  await page.getByRole('combobox', { name: "选择采集记录" }).selectOption('tape-1')
  await expect(page.getByText('#0000', { exact: true })).toBeVisible()
  await expect(page.locator('.event-row')).toHaveCount(100)
  await page.screenshot({ path: testInfo.outputPath('pagination-desktop.png') })
  await page.setViewportSize({ width: 390, height: 844 })
  await expect(page.getByRole('button', { name: "下一页采集记录" })).toBeVisible()
  await page.screenshot({ path: testInfo.outputPath('pagination-mobile.png') })
})

test('navigation opens compile and permission review pages', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#compile?skill=demo')
  await expect(page.getByRole('heading', { name: "已编译技能包" })).toBeVisible()
  await expect(page.getByText("静态检查通过")).toBeVisible()
  await expect(page.getByText('format', { exact: true })).toBeVisible()

  await page.getByRole('link', { name: "权限", exact: true }).click()
  await expect(page.getByRole('heading', { name: "技能包权限" })).toBeVisible()
  await expect(page.getByText("未开放环境变量访问。")).toBeVisible()
  await expect(page.getByText("限定范围的访问", { exact: true })).toBeVisible()
})

test('review navigation asks for saved evidence and keeps the selected Skill', async ({ page }, testInfo) => {
  const requests: string[] = []
  await mockApi(page)
  await page.route('**/api/v1/skills/**/diff', async route => {
    requests.push(route.request().url())
    await route.fulfill({ contentType: 'application/json', body: JSON.stringify({ ...skill, id: 'release #1' }) })
  })
  page.on('request', request => {
    if (request.url().includes('/receipts/')) requests.push(request.url())
  })
  await page.goto('/#compile')
  await expect(page.getByLabel("已保存技能包 ID", { exact: true })).toBeVisible()
  await expect(page.getByRole('heading', { name: "尚未选择技能包" })).toBeVisible()
  await expect(page.getByRole('link', { name: "权限", exact: true })).toHaveAttribute('href', '#permissions')
  expect(requests).toEqual([])
  await page.getByLabel("已保存技能包 ID", { exact: true }).fill('release #1')
  await page.getByRole('button', { name: "打开技能包", exact: true }).click()
  await expect(page.getByText("静态检查通过")).toBeVisible()
  await page.getByRole('link', { name: "权限", exact: true }).click()
  await expect(page.getByLabel("已保存技能包 ID", { exact: true })).toHaveValue('release #1')
  await expect(page.getByRole('heading', { name: "技能包权限" })).toBeVisible()
  await page.reload()
  await expect(page.getByLabel("已保存技能包 ID", { exact: true })).toHaveValue('release #1')
  expect(requests).toHaveLength(3)
  expect(requests.every(url => url.endsWith('/skills/release%20%231/diff'))).toBe(true)
  await page.setViewportSize({ width: 390, height: 844 })
  await expect(page.getByRole('button', { name: "打开技能包", exact: true })).toBeVisible()
  await page.screenshot({ path: testInfo.outputPath('saved-skill-mobile.png') })
  await page.getByRole('link', { name: "查看验证凭据", exact: true }).click()
  await expect(page.getByRole('heading', { name: "尚未选择凭据" })).toBeVisible()
  expect(requests).toHaveLength(3)
})

test('invalid Skill identifiers stay local and a missing package can be corrected', async ({ page }) => {
  const requested: string[] = []
  await mockApi(page)
  await page.route('**/api/v1/skills/**/diff', async route => {
    requested.push(route.request().url())
    if (requested.length === 1) await route.fulfill({ status: 404, contentType: 'application/json', body: JSON.stringify({ error: { message: 'Saved Skill not found. Check the package directory name.' } }) })
    else await route.fulfill({ contentType: 'application/json', body: JSON.stringify(skill) })
  })
  await page.goto('/#permissions?skill=..%2Foutside')
  await expect(page.getByRole('alert').filter({ hasText: '请填写技能包目录名称，不要填写路径。' })).toBeVisible()
  expect(requested).toEqual([])
  await page.getByLabel("已保存技能包 ID", { exact: true }).fill('missing')
  await page.getByRole('button', { name: "打开技能包", exact: true }).click()
  await expect(page.getByRole('alert')).toContainText("未找到这份已保存记录")
  await page.getByLabel("已保存技能包 ID", { exact: true }).fill('demo')
  await page.getByRole('button', { name: "打开技能包", exact: true }).click()
  await expect(page.getByRole('heading', { name: "技能包权限" })).toBeVisible()
  expect(requested).toHaveLength(2)
})

test('receipt view shows bounded evidence and policy decisions', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#receipt?run=run-a')

  await expect(page.getByRole('heading', { name: "验证凭据" })).toBeVisible()
  await expect(page.locator('.page-heading .status-badge').getByText("成功", { exact: true })).toBeVisible()
  await expect(page.getByText("输出摘要与字节数")).toBeVisible()
  await expect(page.locator('.decision-list').getByText('POLICY001', { exact: false })).toBeVisible()
  await expect(page.locator('.result-list').getByText("文件存在", { exact: true })).toBeVisible()
})

test('receipt step table can be focused and scrolled with the keyboard on mobile', async ({ page }) => {
  await mockApi(page)
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/#receipt?run=run-a')
  const region = page.getByRole('region', { name: "步骤结果", exact: true })
  await region.focus()
  await expect(region).toBeFocused()
  expect(await region.evaluate(element => element.scrollWidth > element.clientWidth)).toBe(true)
  await region.press('ArrowRight')
  await expect.poll(() => region.evaluate(element => element.scrollLeft)).toBeGreaterThan(0)
})

test('receipt review filters failed output checks and keeps the complete outcome visible', async ({ page }, testInfo) => {
  await mockApi(page, 'normal', {
    ...receipt, status: 'assertion_failed', assertions: [
      { kind: 'file_exists', target: 'outputs/report.csv', passed: true, reason: 'file exists' },
      { kind: 'file_hash', target: 'outputs/report.csv', passed: false, reason: 'output hash does not match expected digest' },
      { kind: 'command_exit', target: 'export', passed: true, reason: 'exit code matches' },
    ],
  })
  await page.goto('/#receipt?run=run-a')
  await expect(page.getByRole('status').filter({ hasText: '2 通过 · 1 失败' })).toBeVisible()
  await page.getByRole('button', { name: "待处理（1)", exact: true }).click()
  await expect(page.locator('.result-list li')).toHaveCount(1)
  await expect(page.locator('.result-list')).toContainText("文件哈希")
  await expect(page.locator('.result-list')).toContainText("失败")
  await expect(page.locator('.page-heading')).toContainText("断言失败")
  await page.screenshot({ path: testInfo.outputPath('receipt-output-review-desktop.png'), fullPage: true })
  await page.getByRole('button', { name: "全部检查（3)", exact: true }).click()
  await expect(page.locator('.result-list li')).toHaveCount(3)
  await expect(page.locator('.result-list').getByText("通过", { exact: true })).toHaveCount(2)
})

test('receipt next review action focuses failed checks and resets for another saved run', async ({ page }) => {
  await mockApi(page, 'normal', {
    ...receipt, status: 'assertion_failed', assertions: [
      { kind: 'file_exists', target: 'outputs/report.csv', passed: true, reason: 'file exists' },
      { kind: 'file_hash', target: 'outputs/report.csv', passed: false, reason: 'output hash does not match expected digest' },
    ],
  })
  await page.route('**/api/v1/receipts/other-run', route => route.fulfill({ json: {
    schema: 'skilltape.dev/receipt/v1', id: 'other-run', document: receipt,
  } }))
  await page.goto('/#receipt?run=run-a')
  await expect(page.getByRole('region', { name: "下一步需要复核什么" })).toContainText("1 项检查需要处理")
  await page.getByRole('button', { name: "复核失败检查", exact: true }).click()
  await expect(page.getByRole('heading', { name: "预期结果检查" })).toBeFocused()
  await expect(page.locator('.result-list li')).toHaveCount(1)
  await expect(page).toHaveURL(/#receipt\?run=run-a$/)
  await page.getByLabel("已保存凭据 ID", { exact: true }).fill('other-run')
  await page.getByRole('button', { name: "打开凭据", exact: true }).click()
  await expect(page.getByRole('button', { name: "全部检查（1)", exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.locator('.result-list li')).toHaveCount(1)
  await expect(page.getByRole('region', { name: "下一步需要复核什么" })).toContainText("本次运行已完成")
})

test('receipt 未设置显式检查 directs review to execution', async ({ page }) => {
  await mockApi(page, 'normal', { ...receipt, assertions: [] })
  await page.goto('/#receipt?run=run-a')
  await expect(page.getByRole('region', { name: "下一步需要复核什么" })).toContainText('未设置显式检查')
  await page.getByRole('button', { name: "复核步骤结果", exact: true }).click()
  await expect(page.getByRole('heading', { name: "步骤结果" })).toBeFocused()
  await expect(page.getByText("未记录显式断言。", { exact: true })).toBeVisible()
})

test('saved Receipt picker recovers from missing IDs and rejects path input without a request', async ({ page }) => {
  const requests: string[] = []
  page.on('request', request => { if (request.url().includes('/api/v1/receipts/')) requests.push(request.url()) })
  await mockApi(page)
  await page.goto('/#receipt?run=missing')
  await expect(page.getByRole('alert')).toContainText("未找到这份已保存记录")
  const input = page.getByLabel("已保存凭据 ID", { exact: true })
  await input.fill('../private')
  await page.getByRole('button', { name: "打开凭据", exact: true }).click()
  await expect(page.getByText("请填写已保存文件名，不要填写路径。", { exact: true })).toBeVisible()
  expect(requests).toHaveLength(1)
  await input.fill('run-a.json')
  await input.press('Enter')
  await expect(page.locator('.result-list')).toContainText("文件存在")
  await expect(page).toHaveURL(/#receipt\?run=run-a$/)
  await expect(page.getByRole('button', { name: "全部检查（1)", exact: true })).toHaveAttribute('aria-pressed', 'true')
})

test('malformed or oversized stored Receipts show a recoverable error', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  const cases = [
    { ...receipt, assertions: [{ type: 'file_hash', target: 'output', passed: true, reason: 'legacy fixture field' }] },
    { ...receipt, assertions: Array.from({ length: 1001 }, () => receipt.assertions[0]) },
    { ...receipt, steps: [{ ...receipt.steps[0], stdout_bytes: -1 }] },
    { ...receipt, assertions: [{ ...receipt.assertions[0], passed: false }] },
    { ...receipt, raw_output: 'private-sentinel-not-for-display' },
  ]
  for (const document of cases) {
    await page.route('**/api/v1/receipts/**', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify({ schema: receipt.schema, id: 'run-a', document }) }))
    await page.goto('/#receipt?run=run-a')
    await expect(page.getByRole('alert')).toContainText("无效或版本不受支持")
    await expect(page.getByRole('button', { name: "打开凭据", exact: true })).toBeVisible()
    await expect(page.locator('.result-list')).toHaveCount(0)
    await expect(page.locator('body')).not.toContainText('private-sentinel-not-for-display')
    await page.unroute('**/api/v1/receipts/**')
  }
  expect(errors).toEqual([])
})

test('opening a different saved Receipt clears the previous failure filter', async ({ page }) => {
  await mockApi(page, 'normal', { ...receipt, status: 'assertion_failed', assertions: [{ ...receipt.assertions[0], passed: false }] })
  await page.route('**/api/v1/receipts/run-b', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify({ schema: receipt.schema, id: 'run-b', document: { ...receipt, run_id: 'e'.repeat(64) } }) }))
  await page.goto('/#receipt?run=run-a')
  await page.getByRole('button', { name: "待处理（1)", exact: true }).click()
  await page.getByLabel("已保存凭据 ID", { exact: true }).fill('run-b')
  await page.getByRole('button', { name: "打开凭据", exact: true }).click()
  await expect(page.getByRole('button', { name: "全部检查（1)", exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.locator('.result-list')).toContainText("通过")
  await expect(page.locator('.page-heading')).toContainText("成功")
})

test('receipt details render literal untrusted text and remain readable on mobile', async ({ page }, testInfo) => {
  const target = 'outputs/' + 'long-file-name-'.repeat(24) + '<img src=x onerror=alert(1)>.csv'
  const reason = '<script>window.receiptInjected=true</script> digest mismatch'
  await mockApi(page, 'normal', { ...receipt, status: 'assertion_failed', assertions: [{ kind: 'file_hash', target, passed: false, reason }] })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/#receipt?run=run-a')
  await expect(page.locator('.result-list')).toContainText(target)
  await expect(page.locator('.result-list')).toContainText(reason)
  await expect(page.locator('.result-list img, .result-list script')).toHaveCount(0)
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true)
  await page.screenshot({ path: testInfo.outputPath('receipt-output-review-mobile.png'), fullPage: true })
})

test('empty and unavailable states explain the next step', async ({ page }) => {
  await mockApi(page)
  await page.goto('/#timeline?tape=none')
  await expect(page.getByRole('heading', { name: "采集记录为空" })).toBeVisible()

  await mockApi(page, 'error')
  await page.goto('/#timeline?retry=1')
  await expect(page.getByRole('alert')).toContainText('本机控制台暂不可用')
  await expect(page.getByRole('button', { name: "重试" })).toBeVisible()
})


test('permission preflight is bounded, read-only and fails closed on inconsistent plans', async ({ page }, testInfo) => {
  const checks = Array.from({ length: 40 }, (_, index) => ({ ordinal: index + 1, step_id: 'step-' + index, allowed: index !== 35, code: index === 35 ? 'PKG006' : 'POLICY_ALLOWED', reason: index === 35 ? 'path is outside declared filesystem write scopes' : 'allowed', risk: index === 35 ? 'high' : 'low' }))
  let plan = { schema: 'skilltape.dev/policy-plan/v1', execution: 'not-run', allowed: false, package_valid: false, package_error_codes: ['PKG006'], checks }
  const requests: string[] = []
  page.on('request', request => { if (request.url().includes('/api/v1/')) requests.push(request.method()) })
  await mockApi(page)
  await page.route('**/api/v1/skills/demo/diff', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify({ ...skill, policy_plan: plan }) }))
  await page.goto('/#permissions?skill=demo')
  const panel = page.getByRole('article').filter({ has: page.getByRole('heading', { name: "声明策略检查" }) })
  await expect(panel.getByText("已阻断：请复核技能包", { exact: true })).toBeVisible()
  await panel.locator('summary').click()
  await expect(panel.locator('li')).toHaveCount(32)
  await expect(panel).toContainText("显示 1–32 / 40 项检查")
  await panel.getByRole('button', { name: "仅拒绝项（1)", exact: true }).click()
  await expect(panel.locator('li')).toHaveCount(1)
  await expect(panel.locator('li')).toContainText('step-35')
  await page.setViewportSize({ width: 390, height: 844 })
  await page.screenshot({ path: testInfo.outputPath('preflight-denied-mobile.png'), fullPage: true })
  await expect(panel.getByText("已阻断：请复核技能包", { exact: true })).toBeVisible()
  await panel.getByRole('button', { name: "全部检查（40)", exact: true }).click()
  await panel.getByRole('button', { name: "下一页检查", exact: true }).click()
  await expect(panel.locator('li')).toHaveCount(8)
  await expect(panel.locator('li').first()).toContainText('step-32')
  await expect(panel.getByRole('button', { name: "下一页检查", exact: true })).toBeDisabled()
  await panel.getByRole('button', { name: "上一页检查", exact: true }).click()
  await expect(panel.locator('li')).toHaveCount(32)
  await page.setViewportSize({ width: 390, height: 844 })
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
  expect(requests.every(method => method === 'GET')).toBe(true)
  plan = { ...plan, allowed: true }
  await page.reload()
  await expect(page.getByText("策略预检无效或版本不受支持，未批准任何执行。")).toBeVisible()
  await expect(page.getByText("声明的策略允许", { exact: true })).toHaveCount(0)
  plan = { ...plan, package_valid: true, package_error_codes: [], checks: checks.map(check => ({ ...check, allowed: true, code: 'POLICY_ALLOWED', risk: 'low' })) }
  await page.reload()
  await panel.locator('summary').click()
  await panel.getByRole('button', { name: "仅拒绝项（0)", exact: true }).click()
  await expect(panel.getByRole('status')).toHaveText("此计划没有拒绝项。")
  await expect(panel.locator('li')).toHaveCount(0)
  await expect(panel.getByText("声明的策略允许", { exact: true })).toBeVisible()
  expect(requests.every(method => method === 'GET')).toBe(true)
})


test('invalid timeline timestamps show a recoverable error instead of crashing', async ({ page }) => {
  await mockApi(page)
  await page.route('**/api/v1/tapes/tape-a/events*', route => route.fulfill({json: {
    schema: 'skilltape.dev/console/v1', tape_id: 'tape-a', offset: 0, limit: 100, total: 1, next_offset: null,
    events: [{sequence: 0, occurred_at_ms: 9e18, kind: 'terminal_command', source: 'shell', payload: {}, redaction: 'redacted'}],
  }}), {times: 1})
  await page.goto('/#timeline')
  await expect(page.getByRole('alert')).toContainText("已保存时间线无效或版本不受支持")
  await page.getByRole('button', {name: "重试"}).click()
  await expect(page.getByRole('heading', {name: "采集时间线", exact: true})).toBeVisible()
  await expect(page.getByText("已脱敏", {exact: true})).toBeVisible()
})

for (const broken of ['null step', 'invalid permissions', 'invalid file', 'invalid display field'] as const) {
  test(`malformed package ${broken} keeps compile review recoverable`, async ({ page }) => {
    await mockApi(page)
    const payload = broken === 'null step' ? {...skill, workflow: {...skill.workflow, steps: [null]}} :
      broken === 'invalid display field' ? {...skill, workflow: {...skill.workflow, steps: [{id: 'bad', action: 'exec', program: {toString: 'not callable'}}]}} :
      broken === 'invalid permissions' ? {...skill, permissions: {...permissions, filesystem: {read: [null], write: []}}} :
      {...skill, files: [{path: 'workflow.yaml', bytes: 10, sha256: null}]}
    await page.route('**/api/v1/skills/demo/diff', route => route.fulfill({json: payload}), {times: 1})
    await page.goto('/#compile?skill=demo')
    await expect(page.getByRole('alert')).toContainText("已保存技能包无效或版本不受支持")
    await page.getByRole('button', {name: "重试"}).click()
    await expect(page.getByRole('heading', {name: "已编译技能包", exact: true})).toBeVisible()
  })
}

test('an empty policy plan does not imply that checks passed', async ({ page }) => {
  await mockApi(page)
  await page.route('**/api/v1/skills/demo/diff', route => route.fulfill({json: {...skill, policy_plan: {
    schema: 'skilltape.dev/policy-plan/v1', execution: 'not-run', allowed: false, package_valid: false,
    package_error_codes: ['EMPTY_WORKFLOW'], checks: [],
  }}}))
  await page.goto('/#permissions?skill=demo')
  await page.getByText("检查 0 项策略检查", {exact: true}).click()
  await expect(page.getByRole('status')).toHaveText("此计划未记录策略检查。")
})


test('a disconnected Console gives a connection recovery step', async ({ page }) => {
  await mockApi(page)
  await page.route('**/api/v1/workspaces', route => route.abort('failed'), {times: 1})
  await page.goto('/#timeline')
  await expect(page.getByRole('alert')).toContainText('请启动本机控制台后重试。')
  await page.getByRole('button', {name: "重试"}).click()
  await expect(page.getByRole('heading', {name: "采集时间线", exact: true})).toBeVisible()
})

test('compile review distinguishes a recorded Capture source from legacy metadata', async ({page}) => {
  await mockApi(page)
  const source = {format:'skilltape.dev/tape-source/v1',manifest_sha256:'sha256:'+'a'.repeat(64),events_sha256:'sha256:'+'b'.repeat(64),event_count:3}
  await page.route('**/api/v1/skills/demo/diff', route => route.fulfill({json:{...skill,compile_source:source}}), {times:1})
  await page.goto('/#compile?skill=demo')
  await expect(page.getByRole('heading',{name:"采集来源",exact:true})).toBeVisible()
  await expect(page.getByText("记录的来源： 3 个事件。",{exact:false})).toBeVisible()
  await expect(page.getByText(source.events_sha256,{exact:true})).toBeVisible()
  await expect(page.getByText('不认证作者身份',{exact:false})).toBeVisible()
  await page.reload()
  await expect(page.getByText("此技能包未记录采集来源",{exact:false})).toBeVisible()
})


test('capture package lookup shows only exact matches and rejects a response for another Tape',async({page})=>{
 await mockApi(page);let foreign=false;
 await page.route('**/api/v1/tapes/tape-a/packages*',route=>route.fulfill({json:{schema:'skilltape.dev/console/v1',tape_id:foreign?'other':'tape-a',items:[{id:'demo',matches_tape:true,source_state:'matched'},{id:'different',matches_tape:false,source_state:'different'}],offset:0,limit:50,total:2,next_offset:null}}));
 await page.goto('/#timeline');await page.getByRole('button',{name:"查找已编译技能包"}).click();await expect(page.getByRole('link',{name:"复核编译包 demo"})).toHaveAttribute('href','#compile?skill=demo');await expect(page.getByRole('link',{name:"复核编译包 different"})).toHaveCount(0);
 foreign=true;await page.getByRole('button',{name:"刷新技能包查询"}).click();await expect(page.getByText('技能包查询不完整或版本不受支持',{exact:false})).toBeVisible();await expect(page.getByRole('link',{name:"复核编译包 demo"})).toHaveCount(0);
});
