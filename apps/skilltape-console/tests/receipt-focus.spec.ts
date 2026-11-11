import {expect,test} from '@playwright/test'
for(const width of [1440,390,320])test(`opening a saved Receipt brings its result into view at ${width}px`,async({page})=>{
 await page.setViewportSize({width,height:900})
 await page.route('**/api/v1/receipts/focus',route=>route.fulfill({json:{schema:'skilltape.dev/receipt/v1',id:'focus',document:{schema:'skilltape.dev/receipt/v1',run_id:'a'.repeat(64),skill_hash:'b'.repeat(64),status:'succeeded',steps:[],assertions:[],policy_decisions:[]}}}))
 await page.goto('/#receipt');await page.getByLabel('Saved Receipt ID',{exact:true}).fill('focus');await page.getByRole('button',{name:'Open Receipt',exact:true}).click()
 const result=page.getByRole('heading',{name:'Verification receipt',exact:true});await expect(result).toBeFocused();const box=await result.boundingBox();expect(box?.y).toBeGreaterThanOrEqual(0);expect(box?.y).toBeLessThan(100)
})
