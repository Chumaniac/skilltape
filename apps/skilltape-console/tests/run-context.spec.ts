import {expect,test} from '@playwright/test'
import {compareRunContexts,validContextReview,type ContextReview} from '../src/run-context'
const context = {schema:'skilltape.dev/run-context/v1' as const,receipt_sha256:'d'.repeat(64),run_id:'a'.repeat(64),skill_hash:'b'.repeat(64),input_set_sha256:'c'.repeat(64),input_observation:'bounded-before-run' as const,executor:'skilltape-runner/sequential-local-v1',executor_version:'0.1.0',os:'macos',arch:'aarch64',max_processes:1,step_timeout_ms:1000,max_output_bytes:65536}
const review=(change={})=>({schema:'skilltape.dev/context-review/v1',id:'run-a',status:'recorded',context:{...context,...change},provenance:'not-authenticated'}) as ContextReview

test('compares observed input, package, platform and missing conditions independently',()=>{
 const a=review();expect(validContextReview(a,'run-a')).toBe(true)
 expect(compareRunContexts(a,review({input_set_sha256:'f'.repeat(64)}))).toEqual(['Input snapshot changed'])
 expect(compareRunContexts(a,review({skill_hash:'f'.repeat(64)}))).toEqual(['Skill package changed'])
 expect(compareRunContexts(a,review({os:'linux',arch:'x86_64'}))).toEqual(['Platform changed'])
 expect(compareRunContexts(a,{...review(),status:'unrecorded',context:null})).toBeNull()
 expect(compareRunContexts(a,{...review(),status:'invalid',context:null})).toBeNull()
 expect(compareRunContexts(a,review())).toEqual([])
 expect(validContextReview({...a,context:{...context,raw_input:'private'}},'run-a')).toBe(false)
})

test('condition card compares two records without replay and clears stale comparison',async({page})=>{
 let posts=0;page.on('request',r=>{if(r.method()!=='GET')posts++})
 await page.route('**/api/v1/**',route=>{
 const url=new URL(route.request().url())
 if(url.pathname.endsWith('/context'))return route.fulfill({json:url.pathname.includes('baseline')?{...review({input_set_sha256:'f'.repeat(64),os:'linux'}),id:'baseline'}:review()})
 if(url.pathname.endsWith('/receipts/run-a'))return route.fulfill({json:{schema:'skilltape.dev/receipt/v1',id:'run-a',document:{schema:'skilltape.dev/receipt/v1',run_id:context.run_id,skill_hash:context.skill_hash,status:'assertion_failed',steps:[],assertions:[{kind:'file_exists',target:'outputs/a',passed:false,reason:'Missing output'}],policy_decisions:[]}}})
 return route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[],total:0,offset:0,limit:50,next_offset:null}})
 })
 await page.goto('/#receipt?run=run-a')
 await expect(page.getByRole('region',{name:'Run conditions'})).toContainText('Recorded · receipt bytes match')
 await page.getByLabel('Baseline receipt ID').fill('baseline');await page.getByRole('button',{name:'Compare conditions'}).click()
 await expect(page.getByRole('list',{name:'Observed condition differences'})).toContainText('Input snapshot changed')
 await expect(page.getByRole('list',{name:'Observed condition differences'})).toContainText('Platform changed')
 await page.getByLabel('Baseline receipt ID').fill('other');await expect(page.getByRole('list',{name:'Observed condition differences'})).toHaveCount(0)
 expect(posts).toBe(0)
})
