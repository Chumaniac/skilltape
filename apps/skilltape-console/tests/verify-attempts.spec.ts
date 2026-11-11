import {test,expect} from '@playwright/test'

test('standalone attempts have their own readonly receipt-library panel',async({page})=>{
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[],offset:0,limit:12,total:0,next_offset:null}}))
 await page.goto('/#receipt')
 const panel=page.getByRole('region',{name:"单次验证尝试",exact:true})
 await expect(panel).toBeVisible()
 await panel.getByRole('button',{name:"浏览单次尝试",exact:true}).click()
 await expect(panel).toContainText("没有保存单次验证尝试")
 await expect(page.getByRole('button',{name:"比较已保存结果",exact:true})).toBeVisible()
})

const first='a'.repeat(64),second='b'.repeat(64)
const view=(id=first)=>({id,started_at_ms:1000,finished_at_ms:null,outcome:'unconfirmed',receipt:null,record_integrity:'start-only',read_only:true,provenance:'not-authenticated',saved_receipt_verified:false})
const collection=(items:unknown[])=>({schema:'skilltape.dev/console/v1',items,offset:0,limit:12,total:items.length,next_offset:null})

test('incomplete records are never a successful Receipt or a retry control',async({page})=>{
 let writes=0;page.on('request',r=>{if(r.method()!=='GET')writes++})
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([view()])}))
 await page.route('**/api/v1/verify-attempts/'+first,route=>route.fulfill({json:view()}))
 await page.goto('/#receipt');await page.getByRole('button',{name:"浏览单次尝试",exact:true}).click()
 await page.getByRole('button',{name:"读取尝试 "+first,exact:true}).click()
 const result=page.getByRole('region',{name:"单次尝试记录",exact:true})
 await expect(result).toContainText("可能仍在运行或曾被中断")
 await expect(result).toContainText("仅有开始记录")
 await expect(result.getByRole('button')).toHaveCount(0);expect(writes).toBe(0)
 await page.setViewportSize({width:390,height:844});expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
})

test('cancelled reads cannot overwrite a later selected attempt',async({page})=>{
 let release!:()=>void
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([view(),view(second)])}))
 await page.route('**/api/v1/verify-attempts/'+first,async route=>{await new Promise<void>(r=>{release=r});await route.fulfill({json:view()}).catch(()=>{})})
 const prepared={...view(second),outcome:'preparation_failed',record_integrity:'metadata-bound',finished_at_ms:2000}
 await page.route('**/api/v1/verify-attempts/'+second,route=>route.fulfill({json:prepared}))
 await page.goto('/#receipt');await page.getByRole('button',{name:"浏览单次尝试",exact:true}).click()
 await page.getByRole('button',{name:"读取尝试 "+first,exact:true}).click();await page.getByRole('button',{name:"取消读取尝试",exact:true}).click()
 await page.getByRole('button',{name:"读取尝试 "+second,exact:true}).click();const result=page.getByRole('region',{name:"单次尝试记录",exact:true});await expect(result).toContainText(second)
 release();await expect(result).toContainText("未向执行器提交验证");await expect(result).toContainText(second)
})

test('a generated cancelled Receipt stays distinct from saved-file verification',async({page})=>{
 const data={...view(),finished_at_ms:2000,record_integrity:'metadata-bound',outcome:'receipt_produced',receipt:{receipt_sha256:first,run_id:second,skill_hash:first,input_set_sha256:second,status:'cancelled'}}
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([data])}))
 await page.route('**/api/v1/verify-attempts/'+first,route=>route.fulfill({json:data}))
 await page.goto('/#receipt');await page.getByRole('button',{name:"浏览单次尝试",exact:true}).click();await page.getByRole('button',{name:"读取尝试 "+first,exact:true}).click()
 const result=page.getByRole('region',{name:"单次尝试记录",exact:true});await expect(result).toContainText("已取消");await expect(result).toContainText("尚未验证其文件内容");await expect(result).toContainText("不认证作者身份")
})

test('malformed or mismatched attempt responses stay unavailable',async({page})=>{
 await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([view()])}))
 await page.route('**/api/v1/verify-attempts/'+first,route=>route.fulfill({json:view(second)}))
 await page.goto('/#receipt');await page.getByRole('button',{name:"浏览单次尝试",exact:true}).click();await page.getByRole('button',{name:"读取尝试 "+first,exact:true}).click()
 await expect(page.getByText("无法核对单次尝试元数据，已保存文件未被修改。",{exact:true})).toBeVisible();await expect(page.getByRole('region',{name:"单次尝试记录",exact:true})).toHaveCount(0)
 await page.unroute('**/api/v1/verify-attempts?*');await page.route('**/api/v1/verify-attempts?*',route=>route.fulfill({json:collection([{...view(),outcome:'receipt_produced',record_integrity:'metadata-bound',finished_at_ms:2000}])}))
 await page.getByRole('button',{name:"浏览单次尝试",exact:true}).click();await expect(page.getByText("单次尝试记录无效或不完整，已保存文件未被修改。",{exact:true})).toBeVisible()
 await expect(page.getByRole('button',{name:"比较已保存结果",exact:true})).toBeVisible()
})
