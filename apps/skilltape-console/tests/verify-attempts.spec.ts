import {test,expect} from '@playwright/test'

test('standalone attempts have their own readonly receipt-library panel',async({page})=>{
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[],offset:0,limit:12,total:0,next_offset:null}}))
 await page.goto('/#receipt')
 const panel=page.getByRole('region',{name:'Single verification attempts',exact:true})
 await expect(panel).toBeVisible()
 await panel.getByRole('button',{name:'Browse single attempts',exact:true}).click()
 await expect(panel).toContainText('No saved single verification attempts')
 await expect(page.getByRole('button',{name:'Compare saved outcomes',exact:true})).toBeVisible()
})

const first='a'.repeat(64),second='b'.repeat(64)
const view=(id=first)=>({id,started_at_ms:1000,finished_at_ms:null,outcome:'unconfirmed',receipt:null,record_integrity:'start-only',read_only:true,provenance:'not-authenticated',saved_receipt_verified:false})
const collection=(items:unknown[])=>({schema:'skilltape.dev/console/v1',items,offset:0,limit:12,total:items.length,next_offset:null})

test('incomplete records are never a successful Receipt or a retry control',async({page})=>{
 let writes=0;page.on('request',r=>{if(r.method()!=='GET')writes++})
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([view()])}))
 await page.route('**/api/v1/verify-attempts/'+first,route=>route.fulfill({json:view()}))
 await page.goto('/#receipt');await page.getByRole('button',{name:'Browse single attempts',exact:true}).click()
 await page.getByRole('button',{name:'Read attempt '+first,exact:true}).click()
 const result=page.getByRole('region',{name:'Single attempt record',exact:true})
 await expect(result).toContainText('may still be active or may have been interrupted')
 await expect(result).toContainText('Only a start record')
 await expect(result.getByRole('button')).toHaveCount(0);expect(writes).toBe(0)
 await page.setViewportSize({width:390,height:844});expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
})

test('cancelled reads cannot overwrite a later selected attempt',async({page})=>{
 let release!:()=>void
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([view(),view(second)])}))
 await page.route('**/api/v1/verify-attempts/'+first,async route=>{await new Promise<void>(r=>{release=r});await route.fulfill({json:view()}).catch(()=>{})})
 const prepared={...view(second),outcome:'preparation_failed',record_integrity:'metadata-bound',finished_at_ms:2000}
 await page.route('**/api/v1/verify-attempts/'+second,route=>route.fulfill({json:prepared}))
 await page.goto('/#receipt');await page.getByRole('button',{name:'Browse single attempts',exact:true}).click()
 await page.getByRole('button',{name:'Read attempt '+first,exact:true}).click();await page.getByRole('button',{name:'Cancel attempt read',exact:true}).click()
 await page.getByRole('button',{name:'Read attempt '+second,exact:true}).click();const result=page.getByRole('region',{name:'Single attempt record',exact:true});await expect(result).toContainText(second)
 release();await expect(result).toContainText('No verification was submitted to the runner');await expect(result).toContainText(second)
})

test('a generated cancelled Receipt stays distinct from saved-file verification',async({page})=>{
 const data={...view(),finished_at_ms:2000,record_integrity:'metadata-bound',outcome:'receipt_produced',receipt:{receipt_sha256:first,run_id:second,skill_hash:first,input_set_sha256:second,status:'cancelled'}}
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([data])}))
 await page.route('**/api/v1/verify-attempts/'+first,route=>route.fulfill({json:data}))
 await page.goto('/#receipt');await page.getByRole('button',{name:'Browse single attempts',exact:true}).click();await page.getByRole('button',{name:'Read attempt '+first,exact:true}).click()
 const result=page.getByRole('region',{name:'Single attempt record',exact:true});await expect(result).toContainText('cancelled');await expect(result).toContainText('saved file has not been verified');await expect(result).toContainText('do not authenticate')
})

test('malformed or mismatched attempt responses stay unavailable',async({page})=>{
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([view()])}))
 await page.route('**/api/v1/verify-attempts/'+first,route=>route.fulfill({json:view(second)}))
 await page.goto('/#receipt');await page.getByRole('button',{name:'Browse single attempts',exact:true}).click();await page.getByRole('button',{name:'Read attempt '+first,exact:true}).click()
 await expect(page.getByText('Single attempt metadata could not be checked. Saved files were not changed.',{exact:true})).toBeVisible();await expect(page.getByRole('region',{name:'Single attempt record',exact:true})).toHaveCount(0)
 await page.unroute('**/api/v1/verify-attempts?*');await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([{...view(),outcome:'receipt_produced',record_integrity:'metadata-bound',finished_at_ms:2000}])}))
 await page.getByRole('button',{name:'Browse single attempts',exact:true}).click();await expect(page.getByText('Single attempt records are invalid or incomplete. Saved files were not changed.',{exact:true})).toBeVisible()
 await expect(page.getByRole('button',{name:'Compare saved outcomes',exact:true})).toBeVisible()
})
