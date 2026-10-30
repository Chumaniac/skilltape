import {expect,test} from '@playwright/test'
const skill={schema:'skilltape.dev/console/v1',id:'demo',package_path:'skills/demo',manifest:{name:'demo'},workflow:{steps:[]},permissions:{filesystem:{read:[],write:[]},process:{executables:[],max_processes:1,default_timeout_ms:1000},network:{enabled:false,allow_hosts:[]},secrets:{read_environment:false}},lockfile:{},files:[],lint:{files_checked:6,errors:[],warnings:[]}}
test('a reviewed package carries its real path through permissions and export',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}))
 await page.goto('/#compile?skill=demo')
 await page.getByRole('link',{name:'Review this package’s permissions'}).click()
 await expect(page).toHaveURL(/#permissions\?skill=demo$/)
 await page.getByRole('link',{name:'Prepare this package’s export'}).click()
 await expect(page).toHaveURL(/#export\?skill=demo$/)
 await expect(page.getByRole('region',{name:'Export command'})).toContainText("'skills/demo'")
 await expect(page.getByRole('region',{name:'Export command'})).not.toContainText('./my-skill')
 await expect(page.getByRole('region',{name:'Verification command'})).toContainText('--receipt')
 await expect(page.getByRole('button',{name:'Check this verification Receipt'})).toBeVisible();await expect(page.getByRole('link',{name:'View this verification Receipt'})).toHaveCount(0)
})

test('export paths are literal and failed Receipts do not become export evidence',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}))
 await page.route('**/api/v1/receipts/failed',route=>route.fulfill({json:{schema:'skilltape.dev/receipt/v1',id:'failed',document:{schema:'skilltape.dev/receipt/v1',run_id:'a'.repeat(64),skill_hash:'b'.repeat(64),status:'run_failed',steps:[],assertions:[],policy_decisions:[]}}}))
 await page.goto('/#export?skill=demo')
 await page.getByLabel('Export target',{exact:true}).selectOption('codex')
 await page.getByLabel('New output directory').fill("out/a'$(touch nope)")
 await expect(page.getByRole('region',{name:'Export command'})).toContainText("a'\\''$(touch nope)")
 await page.getByLabel('New output directory').fill('')
 await expect(page.getByRole('region',{name:'Export command'})).toHaveCount(0)
 await page.getByLabel('New output directory').fill('out/new')
 await page.getByLabel('Include a saved Receipt',{exact:true}).check()
 await page.getByLabel('Saved Receipt ID',{exact:true}).fill('failed')
 await page.getByRole('button',{name:'Read saved Receipt',exact:true}).click()
 await expect(page.getByText('This Receipt did not succeed.',{exact:false})).toBeVisible()
 await expect(page.getByRole('region',{name:'Export command'})).toHaveCount(0)
})


test('prepared verification identity survives reload and leaving the export view',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}));await page.goto('/#export?skill=demo');
 const command=page.getByRole('region',{name:'Verification command'}).locator('code');await expect(command).toContainText('--receipt');const original=await command.innerText();
 await page.reload();await expect(command).toHaveText(original);await page.getByRole('link',{name:'Return to this package’s review'}).click();
 await page.getByRole('link',{name:'Prepare this package’s export'}).click();await expect(command).toHaveText(original);
});


test('only a checked Receipt can be opened and a new preparation preserves the old evidence',async({page})=>{
 await page.route('**/api/v1/skills/demo/diff',route=>route.fulfill({json:skill}));let reads=0;
 await page.route('**/api/v1/receipts/*',route=>{reads++;return route.fulfill({status:404,json:{error:{code:'not_found',message:'Receipt is not saved yet. Run Verify, then check again.'}}})});await page.goto('/#export?skill=demo');
 const command=page.getByRole('region',{name:'Verification command'}).locator('code');await expect(command).toContainText('--receipt');const original=await command.innerText();
 await page.getByRole('button',{name:'Check this verification Receipt'}).click();await expect(page.getByText('Receipt is not saved yet.',{exact:false}).first()).toBeVisible();await expect(page.getByRole('link',{name:'View this verification Receipt'})).toHaveCount(0);expect(reads).toBe(1);
 await page.getByRole('button',{name:'Prepare a new verification'}).click();await expect(command).not.toHaveText(original);expect(reads).toBe(1);
});
