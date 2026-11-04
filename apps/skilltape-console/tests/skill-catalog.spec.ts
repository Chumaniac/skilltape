import {expect,test} from '@playwright/test'

const ids=Array.from({length:53},(_,index)=>'saved-'+String(index).padStart(2,'0'))
test('saved Skill catalog searches and pages without running or writing a package',async({page})=>{
 let writes=0
 await page.route('**/api/v1/**',route=>{
  if(route.request().method()!=='GET')writes++
  const url=new URL(route.request().url())
  if(url.pathname==='/api/v1/skills'){
   const query=(url.searchParams.get('q')||'').trim().toLowerCase(),offset=Number(url.searchParams.get('offset')||0)
   const matched=ids.filter(id=>id.includes(query)),items=matched.slice(offset,offset+50),next=offset+items.length
   return route.fulfill({json:{schema:'skilltape.dev/console/v1',items,offset,limit:50,total:matched.length,next_offset:next<matched.length?next:null}})
  }
  if(url.pathname==='/api/v1/workspaces')return route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[],offset:0,limit:1,total:0,next_offset:null}})
  return route.fulfill({status:404,json:{error:{message:'Synthetic package document is not part of this catalog fixture.'}}})
 })
 await page.goto('/#compile')
 await page.getByText('Browse saved Skills',{exact:true}).click()
 const list=page.getByRole('list',{name:'Saved Skills'})
 await expect(list.getByRole('button')).toHaveCount(50)
 await page.getByRole('button',{name:'Next saved Skills page'}).click();await expect(list.getByRole('button')).toHaveCount(3)
 await page.getByRole('searchbox',{name:'Search saved Skills'}).fill('SAVED-02')
 await expect(list.getByRole('button')).toHaveCount(1)
 await list.getByRole('button',{name:'saved-02',exact:true}).click()
 await expect(page).toHaveURL(/#compile\?skill=saved-02$/)
 await expect(page.getByLabel('Saved Skill ID',{exact:true})).toHaveValue('saved-02')
 expect(writes).toBe(0)
})

test('catalog failures retain manual ID opening and invalid responses are refused',async({page})=>{
 let invalid=false
 await page.route('**/api/v1/skills?**',route=>invalid?route.fulfill({json:{schema:'skilltape.dev/console/v1',items:['../private'],offset:0,limit:50,total:1,next_offset:null}}):route.fulfill({status:503,json:{error:{code:'unavailable',message:'Synthetic list unavailable'}}}))
 await page.route('**/api/v1/workspaces',route=>route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[],offset:0,limit:1,total:0,next_offset:null}}))
 await page.goto('/#compile')
 await page.getByText('Browse saved Skills',{exact:true}).click()
 await expect(page.getByText('Saved Skills could not be listed. Retry the list, or enter an ID above.')).toBeVisible()
 invalid=true;await page.getByRole('button',{name:'Retry saved Skills'}).click()
 await expect(page.getByRole('list',{name:'Saved Skills'})).toHaveCount(0)
 await page.getByLabel('Saved Skill ID',{exact:true}).fill('known-id')
 await page.getByRole('button',{name:'Open Skill',exact:true}).click()
 await expect(page).toHaveURL(/#compile\?skill=known-id$/)
})
