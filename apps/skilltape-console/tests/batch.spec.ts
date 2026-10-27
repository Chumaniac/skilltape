import { expect, test } from '@playwright/test'

const attempt=(id:string,number:number,status:string)=>({number,status,input_sha256:'c'.repeat(64),receipt_id:`${id}-a${number}`,delivery_id:status==='succeeded'?`${id}-a${number}`:null})
const report=()=>({schema:'skilltape.dev/batch-review/v1',revision:5,status:'partial',package_sha256:'a'.repeat(64),plan_sha256:'b'.repeat(64),max_cases:16,max_attempts:3,provenance:'local-evidence-not-authenticated',cases:[
 {id:'accepted',status:'succeeded',attempt_count:1,input_sha256:'c'.repeat(64),receipt_id:'accepted-a1',delivery_id:'accepted-a1',evidence:'recorded',error_code:null,attempts:[attempt('accepted',1,'succeeded')]},
 {id:'repair',status:'failed',attempt_count:2,input_sha256:'c'.repeat(64),receipt_id:'repair-a2',delivery_id:null,evidence:'recorded',error_code:null,attempts:[attempt('repair',1,'failed'),attempt('repair',2,'failed')]},
]})

test('batch review filters failures and keeps current and earlier attempt links read-only',async({page})=>{
 const methods:string[]=[]
 await page.route('**/api/v1/**',route=>{methods.push(route.request().method());return route.fulfill({json:report()})})
 await page.goto('/#batch')
 await expect(page.getByRole('heading',{name:'Review a verification batch'})).toBeVisible()
 await expect(page.locator('[data-batch-case]')).toHaveCount(2)
 await page.getByLabel('Only cases needing attention').check()
 await expect(page.locator('[data-batch-case]')).toHaveCount(1)
 await expect(page.getByRole('link',{name:'Open Receipt for repair'})).toHaveAttribute('href','#receipt?run=repair-a2')
 await page.getByText('Earlier attempts',{exact:true}).click()
 await expect(page.getByRole('link',{name:'Receipt for repair attempt 1'})).toHaveAttribute('href','#receipt?run=repair-a1')
 await page.getByRole('button',{name:'Refresh batch'}).click()
 await expect(page.locator('[data-batch-case]')).toHaveCount(1)
 expect(methods.every(method=>method==='GET')).toBe(true)
})

test('an inconsistent aggregate cannot claim a completed batch and can recover',async({page})=>{
 let invalid=true
 await page.route('**/api/v1/**',route=>route.fulfill({json:invalid?{...report(),status:'complete'}:report()}))
 await page.goto('/#batch')
 await expect(page.getByRole('alert')).toContainText('invalid or unsupported')
 invalid=false;await page.getByRole('button',{name:'Retry request'}).click()
 await expect(page.locator('[data-batch-case]')).toHaveCount(2)
})

test('a case with a different current-input binding cannot become reviewed evidence',async({page})=>{
 const data=report();data.cases[0].input_sha256='d'.repeat(64)
 await page.route('**/api/v1/**',route=>route.fulfill({json:data}))
 await page.goto('/#batch');await expect(page.getByRole('alert')).toContainText('invalid or unsupported')
 await expect(page.locator('[data-batch-case]')).toHaveCount(0)
})


test('case selection mounts one evidence view, moves focus and avoids retry guidance for changed evidence',async({page})=>{
 const data=report();data.status='unconfirmed';data.cases[1].evidence='changed';data.cases[1].status='unconfirmed';data.cases[1].error_code='evidence-changed'
 await page.route('**/api/v1/**',route=>route.fulfill({json:data}))
 await page.goto('/#batch')
 await expect(page.getByLabel('Selected case repair',{exact:true})).toBeVisible()
 await expect(page.getByText('Saved evidence changed or is missing.',{exact:false})).toBeVisible()
 await expect(page.locator('.batch-case-detail')).toContainText('skilltape batch inspect')
 await expect(page.locator('.batch-case-detail')).not.toContainText('skilltape batch retry')
 await page.getByRole('button',{name:'Review case accepted',exact:true}).click()
 await expect(page.getByLabel('Selected case accepted',{exact:true})).toBeVisible()
 await expect(page.locator('.batch-case-detail h2')).toBeFocused()
 await expect(page.locator('.batch-case-detail')).toHaveCount(1)
 await expect(page.getByRole('link',{name:'Open Receipt for repair',exact:true})).toHaveCount(0)
 await page.setViewportSize({width:390,height:844})
 await page.getByRole('button',{name:'Review case repair',exact:true}).scrollIntoViewIfNeeded()
 await expect(page.getByRole('button',{name:'Review case repair',exact:true})).toBeInViewport()
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
})
