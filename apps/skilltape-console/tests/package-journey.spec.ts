import {expect,test} from '@playwright/test'
const skill={schema:'skilltape.dev/console/v1',id:'demo',package_path:'skills/demo',manifest:{name:'demo'},workflow:{steps:[]},permissions:{filesystem:{read:[],write:[]},process:{executables:[],max_processes:1,default_timeout_ms:1000},network:{enabled:false,allow_hosts:[]},secrets:{read_environment:false}},lockfile:{},files:[],lint:{files_checked:6,errors:[],warnings:[]}}
test('a reviewed package carries its real path through permissions and export',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}))
 await page.goto('/#compile?skill=demo')
 await page.getByRole('link',{name:"复核此技能包权限"}).click()
 await expect(page).toHaveURL(/#permissions\?skill=demo$/)
 await page.getByRole('link',{name:"准备导出此技能包"}).click()
 await expect(page).toHaveURL(/#export\?skill=demo$/)
 await expect(page.getByRole('region',{name:"导出命令"})).toContainText("'skills/demo'")
 await expect(page.getByRole('region',{name:"导出命令"})).not.toContainText('./my-skill')
 await expect(page.getByRole('region',{name:"验证命令"})).toContainText('--receipt')
 await expect(page.getByRole('button',{name:"检查本次验证凭据"})).toBeVisible();await expect(page.getByRole('link',{name:"查看本次验证凭据"})).toHaveCount(0)
})

test('export paths are literal and failed Receipts do not become export evidence',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}))
 await page.route('**/api/v1/receipts/failed',route=>route.fulfill({json:{schema:'skilltape.dev/receipt/v1',id:'failed',document:{schema:'skilltape.dev/receipt/v1',run_id:'a'.repeat(64),skill_hash:'b'.repeat(64),status:'run_failed',steps:[],assertions:[],policy_decisions:[]}}}))
 await page.goto('/#export?skill=demo')
 await page.getByLabel("导出目标",{exact:true}).selectOption('codex')
 await page.getByLabel("新建输出目录").fill("out/a'$(touch nope)")
 await expect(page.getByRole('region',{name:"导出命令"})).toContainText("a'\\''$(touch nope)")
 await page.getByLabel("新建输出目录").fill('')
 await expect(page.getByRole('region',{name:"导出命令"})).toHaveCount(0)
 await page.getByLabel("新建输出目录").fill('out/new')
 await page.getByLabel("附带已保存凭据",{exact:true}).check()
 await page.getByLabel("已保存凭据 ID",{exact:true}).fill('failed')
 await page.getByRole('button',{name:"读取已保存凭据",exact:true}).click()
 await expect(page.getByText("此凭据未成功",{exact:false})).toBeVisible()
 await expect(page.getByRole('region',{name:"导出命令"})).toHaveCount(0)
})


test('prepared verification identity survives reload and leaving the export view',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}));await page.goto('/#export?skill=demo');
 const command=page.getByRole('region',{name:"验证命令"}).locator('code');await expect(command).toContainText('--receipt');const original=await command.innerText();
 await page.reload();await expect(command).toHaveText(original);await page.getByRole('link',{name:"返回此技能包的复核"}).click();
 await page.getByRole('link',{name:"准备导出此技能包"}).click();await expect(command).toHaveText(original);
});


test('only a checked Receipt can be opened and a new preparation preserves the old evidence',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}));let reads=0;
 await page.route('**/api/v1/receipts/*',route=>{reads++;return route.fulfill({status:404,json:{error:{code:'not_found',message:'Receipt is not saved yet. Run Verify, then check again.'}}})});await page.goto('/#export?skill=demo');
 const command=page.getByRole('region',{name:"验证命令"}).locator('code');await expect(command).toContainText('--receipt');const original=await command.innerText();
 await page.getByRole('button',{name:"检查本次验证凭据"}).click();await expect(page.getByText("未找到这份已保存记录",{exact:false}).first()).toBeVisible();await expect(page.getByRole('link',{name:"查看本次验证凭据"})).toHaveCount(0);expect(reads).toBe(1);
 await page.getByRole('button',{name:"准备新的验证"}).click();await expect(command).not.toHaveText(original);expect(reads).toBe(1);
});


test('saved Receipt library reads selected evidence and keeps failed evidence out of export',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',r=>r.fulfill({json:skill}));await page.route('**/api/v1/receipts?*',r=>r.fulfill({json:{schema:'skilltape.dev/console/v1',items:[{id:'known-failed',metadata_state:'available',status:'assertion_failed',skill_hash:'b'.repeat(64),run_id:'a'.repeat(64),bytes:100,modified_at_ms:1710000000000}],offset:0,limit:50,total:1,next_offset:null}}));await page.route('**/api/v1/receipts/known-failed',r=>r.fulfill({json:{schema:'skilltape.dev/receipt/v1',id:'known-failed',document:{schema:'skilltape.dev/receipt/v1',run_id:'a'.repeat(64),skill_hash:'b'.repeat(64),status:'assertion_failed',steps:[],assertions:[],policy_decisions:[]}}}));
 await page.goto('/#export?skill=demo');await page.getByRole('button',{name:"浏览已保存凭据"}).click();await page.getByRole('button',{name:"读取凭据 known-failed",exact:true}).click();await expect(page.getByLabel("已保存凭据 ID",{exact:true})).toHaveValue('known-failed');await expect(page.getByRole('region',{name:"导出命令"})).toHaveCount(0);await expect(page.getByText("此凭据未成功",{exact:false})).toBeVisible();
});

test('saved Receipt filters apply before paging and a failed refresh keeps the previous selection', async ({page}) => {
  const queries: URLSearchParams[] = []
  let fail = false
  await page.route('**/api/v1/skills/demo/diff', route => route.fulfill({json: skill}))
  await page.route('**/api/v1/receipts?*', route => {
    const query = new URL(route.request().url()).searchParams
    queries.push(query)
    if (fail) return route.fulfill({status: 503, json: {error: {code: 'busy', message: '本机控制台暂不可用'}}})
    const offset = Number(query.get('offset'))
    return route.fulfill({json: {schema: 'skilltape.dev/console/v1', items: [{id: offset ? 'later-failure' : 'first-failure', metadata_state: 'available', status: 'run_failed', skill_hash: 'b'.repeat(64), run_id: 'a'.repeat(64), bytes: 100, modified_at_ms: null}], offset, limit: 50, total: 51, next_offset: offset ? null : 50}})
  })
  await page.goto('/#export?skill=demo')
  await page.getByLabel("已记录凭据状态").selectOption('run_failed')
  await page.getByLabel("技能包指纹前缀").fill('BB')
  await page.getByRole('button', {name: "应用凭据筛选"}).click()
  await expect(page.getByRole('button', {name: "读取凭据 first-failure", exact: true})).toBeVisible()
  expect(queries[0].get('status')).toBe('run_failed')
  expect(queries[0].get('skill_hash')).toBe('bb')
  await page.getByLabel("已记录凭据状态").selectOption('succeeded')
  await page.getByRole('button', {name: "下一页凭据", exact: true}).click()
  await expect(page.getByRole('button', {name: "读取凭据 later-failure", exact: true})).toBeVisible()
  expect(queries[1].get('status')).toBe('run_failed')
  expect(queries[1].get('offset')).toBe('50')
  fail = true
  await page.getByRole('button', {name: "应用凭据筛选"}).click()
  await expect(page.getByText('本机控制台暂不可用', {exact: false})).toBeVisible()
  await expect(page.getByLabel("已记录凭据状态")).toHaveValue('run_failed')
  await expect(page.getByRole('button', {name: "读取凭据 later-failure", exact: true})).toBeVisible()
  const reads = queries.length
  await page.getByLabel("技能包指纹前缀").fill('../private')
  await page.getByRole('button', {name: "应用凭据筛选"}).click()
  await expect(page.getByText("指纹最多包含64位十六进制字符。", {exact: false})).toBeVisible()
  expect(queries).toHaveLength(reads)
  await page.setViewportSize({width: 390, height: 844})
  await expect(page.getByRole('button', {name: "应用凭据筛选"})).toBeInViewport()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true)
})
