import { expect, test } from '@playwright/test'

const coverage = (id='a') => ({schema:'skilltape.dev/tape-coverage/v1',tape_id:id,state:id==='a'?'complete':'open',total_events:id==='a'?251:0,
  redaction:{redacted:id==='a'?200:0,unredacted:id==='a'?50:0,partially_redacted:id==='a'?1:0},
  by_kind:id==='a'?{terminal_command:249,capture_warning:2}:{},by_source:id==='a'?{shell:249,system:2}:{},capture_warnings:id==='a'?2:0,
  first_event_at_ms:id==='a'?10:null,last_event_at_ms:id==='a'?260:null,
  samples:id==='a'?[...Array.from({length:31},(_,sequence)=>({sequence,redaction:'unredacted',capture_warning:false})),{sequence:150,redaction:'partially_redacted',capture_warning:false}]:[],
  sample_total:id==='a'?53:0,samples_truncated:id==='a',limits:{max_events:100000,max_bytes:16777216,sample_limit:32}})

async function fixture(page:import('@playwright/test').Page, read:(id:string,route:import('@playwright/test').Route)=>Promise<void>) {
  await page.route('**/api/v1/**',async route=>{
    const url=new URL(route.request().url()),id=url.pathname.split('/').at(-2)!
    if(url.pathname.endsWith('/coverage'))return read(id,route)
    if(url.pathname.endsWith('/workspaces'))return route.fulfill({json:{schema:'skilltape.dev/console/v1',items:[{id:'default',name:'synthetic',tape_count:2,skill_count:0,run_count:0,receipt_count:0}],offset:0,limit:1,total:1,next_offset:null}})
    if(url.pathname.endsWith('/tapes'))return route.fulfill({json:{schema:'skilltape.dev/console/v1',items:['a','b'].map(id=>({id,schema:'skilltape.dev/tape/v1',started_at_ms:10,finished_at_ms:id==='a'?300:null,platform:'test',workspace_root:'relative',event_count:id==='a'?251:0})),offset:0,limit:50,total:2,next_offset:null}})
    const offset=Number(url.searchParams.get('offset')||0),total=id==='a'?251:0;
    return route.fulfill({json:{schema:'skilltape.dev/console/v1',tape_id:id,events:Array.from({length:Math.max(0,Math.min(100,total-offset))},(_,i)=>({sequence:offset+i,occurred_at_ms:10+offset+i,kind:'terminal_command',source:'shell',payload:{summary:'Synthetic event'},redaction:'redacted'})),offset,limit:100,total,next_offset:offset+100<total?offset+100:null}})
  })
}

test('whole capture coverage is on demand, includes all pages and locates an observed sequence',async({page})=>{
  let reads=0;await fixture(page,async(id,route)=>{reads++;await route.fulfill({json:coverage(id)})});
  await page.goto('/#timeline?tape=a');await expect(page.getByRole('heading',{name:'Capture timeline',exact:true})).toBeVisible();expect(reads).toBe(0)
  await page.getByRole('button',{name:'Check whole Tape',exact:true}).click();const panel=page.getByRole('region',{name:'Capture coverage',exact:true});
  await expect(panel).toContainText('251 recorded events');await expect(panel).toContainText('200');await expect(panel).toContainText('32 of 53');await expect(panel).toContainText('metadata cannot establish');expect(reads).toBe(1)
  await panel.getByRole('link',{name:'View sequence 150',exact:true}).click();await expect(page).toHaveURL(/eventsOffset=100/);await expect(page.getByText('#0150', {exact:true}).first()).toBeVisible();expect(reads).toBe(1)
})

test('invalid coverage cannot be presented as a checked capture and can recover',async({page})=>{
  let invalid=true;await fixture(page,async(id,route)=>route.fulfill({json:invalid?{...coverage(id),total_events:1}:coverage(id)}));await page.goto('/#timeline?tape=a');await page.getByRole('button',{name:'Check whole Tape',exact:true}).click();
  const panel=page.getByRole('region',{name:'Capture coverage',exact:true});await expect(panel.getByRole('alert')).toContainText('invalid or unsupported');invalid=false;await panel.getByRole('button',{name:'Retry request',exact:true}).click();await expect(panel).toContainText('251 recorded events')
})

test('switching tapes discards late coverage without scanning the new Tape automatically',async({page})=>{
  let release:()=>void=()=>{};const gate=new Promise<void>(resolve=>release=resolve);let aStarted=false,bReads=0;
  await fixture(page,async(id,route)=>{if(id==='a'){aStarted=true;await gate;}else bReads++;await route.fulfill({json:coverage(id)}).catch(()=>{})});
  await page.goto('/#timeline?tape=a');await page.getByRole('button',{name:'Check whole Tape',exact:true}).click();await expect.poll(()=>aStarted).toBe(true);await page.getByLabel('Select capture tape').selectOption('b');
  await expect(page.getByRole('button',{name:'Check whole Tape',exact:true})).toBeVisible();expect(bReads).toBe(0);await page.getByRole('button',{name:'Check whole Tape',exact:true}).click();release();
  const panel=page.getByRole('region',{name:'Capture coverage',exact:true});await expect(panel).toContainText('0 recorded events');await expect(panel).not.toContainText('251 recorded events')
})
