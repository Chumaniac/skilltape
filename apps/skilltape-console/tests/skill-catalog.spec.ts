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
 await page.getByText("浏览已保存技能包",{exact:true}).click()
 const list=page.getByRole('list',{name:"已保存技能包"})
 await expect(list.getByRole('button')).toHaveCount(50)
 await page.getByRole('button',{name:"下一页已保存技能包"}).click();await expect(list.getByRole('button')).toHaveCount(3)
 await page.getByRole('searchbox',{name:"搜索已保存技能包"}).fill('SAVED-02')
 await expect(list.getByRole('button')).toHaveCount(1)
 await list.getByRole('button',{name:'saved-02',exact:true}).click()
 await expect(page).toHaveURL(/#compile\?skill=saved-02$/)
 await expect(page.getByLabel("已保存技能包 ID",{exact:true})).toHaveValue('saved-02')
 expect(writes).toBe(0)
})

test('catalog failures retain manual ID opening and invalid responses are refused',async({page})=>{
 let invalid=false
 await page.route('**/api/v1/skills?**',route=>invalid?route.fulfill({json:{schema:'skilltape.dev/console/v1',items:['../private'],offset:0,limit:50,total:1,next_offset:null}}):route.fulfill({status:503,json:{error:{code:'unavailable',message:'Synthetic list unavailable'}}}))
 await page.route('**/api/v1/workspaces',route=>route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[],offset:0,limit:1,total:0,next_offset:null}}))
 await page.goto('/#compile')
 await page.getByText("浏览已保存技能包",{exact:true}).click()
 await expect(page.getByText("无法列出已保存技能包，请重试列表或在上方输入 ID。")).toBeVisible()
 invalid=true;await page.getByRole('button',{name:"重新读取技能包列表"}).click()
 await expect(page.getByRole('list',{name:"已保存技能包"})).toHaveCount(0)
 await page.getByLabel("已保存技能包 ID",{exact:true}).fill('known-id')
 await page.getByRole('button',{name:"打开技能包",exact:true}).click()
 await expect(page).toHaveURL(/#compile\?skill=known-id$/)
})
