export type BatchStatus = 'pending' | 'running' | 'succeeded' | 'failed' | 'cancelled' | 'unconfirmed'
export interface BatchAttempt { number:number; status:BatchStatus; input_sha256:string; receipt_id:string|null; delivery_id:string|null }
export interface BatchCase { id:string; status:BatchStatus; attempt_count:number; input_sha256:string; receipt_id:string|null; delivery_id:string|null; evidence:'recorded'|'unrecorded'|'changed'; error_code:string|null; attempts:BatchAttempt[] }
export interface BatchReview { schema:'skilltape.dev/batch-review/v1'; revision:number; status:'pending'|'partial'|'complete'|'unconfirmed'; package_sha256:string; plan_sha256:string; max_cases:16; max_attempts:3; provenance:'local-evidence-not-authenticated'; cases:BatchCase[] }
const record=(value:unknown):value is Record<string,unknown>=>!!value && typeof value==='object' && !Array.isArray(value)
const count=(value:unknown,max:number)=>Number.isSafeInteger(value)&&Number(value)>=0&&Number(value)<=max
const digest=(value:unknown)=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value)
const statuses:BatchStatus[]=['pending','running','succeeded','failed','cancelled','unconfirmed']
const status=(value:unknown)=>typeof value==='string'&&statuses.includes(value as BatchStatus)
function reference(value:unknown,id:string,attempt:number){return value===null || attempt>0 && value===`${id}-a${attempt}`}

export function validBatchReview(value:unknown):value is BatchReview {
 if(!record(value)||value.schema!=='skilltape.dev/batch-review/v1'||!count(value.revision,Number.MAX_SAFE_INTEGER)||!value.revision||
  !digest(value.package_sha256)||!digest(value.plan_sha256)||value.max_cases!==16||value.max_attempts!==3||value.provenance!=='local-evidence-not-authenticated'||
  !Array.isArray(value.cases)||!value.cases.length||value.cases.length>16)return false
 const ids=new Set<string>()
 for(const row of value.cases){
  if(!record(row)||typeof row.id!=='string'||!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,47}$/.test(row.id)||ids.has(row.id)||!status(row.status)||
   !count(row.attempt_count,3)||!digest(row.input_sha256)||!['recorded','unrecorded','changed'].includes(String(row.evidence))||
   ![null,'execution-unconfirmed','evidence-changed'].includes(row.error_code as null|string)||!Array.isArray(row.attempts)||row.attempts.length!==row.attempt_count)return false
  ids.add(row.id)
  const number=Number(row.attempt_count)
  if(!reference(row.receipt_id,row.id,number)||!reference(row.delivery_id,row.id,number)||row.status==='succeeded'&&row.evidence!=='recorded'||row.evidence==='changed'&&row.status!=='unconfirmed')return false
  if(number===0&&(row.status!=='pending'||row.evidence!=='unrecorded'))return false
  for(const [index,attempt] of row.attempts.entries()){
   if(!record(attempt)||attempt.number!==index+1||!status(attempt.status)||attempt.status==='pending'||!digest(attempt.input_sha256)||
    !reference(attempt.receipt_id,row.id,index+1)||!reference(attempt.delivery_id,row.id,index+1)||
    index+1<number&&!['failed','cancelled'].includes(String(attempt.status)))return false
  }
  if(number>0){
   const last=row.attempts.at(-1) as Record<string,unknown>
   if(last.input_sha256!==row.input_sha256||last.receipt_id!==row.receipt_id||last.delivery_id!==row.delivery_id||
    row.status!==last.status&&!(row.status==='unconfirmed'&&row.evidence==='changed'))return false
   if(['succeeded','failed','cancelled'].includes(String(last.status))&&last.receipt_id===null||last.status==='succeeded'&&last.delivery_id===null)return false
  }
 }
 const cases=value.cases as BatchCase[]
 const expected=cases.every(row=>row.status==='succeeded')?'complete':cases.some(row=>row.status==='running'||row.status==='unconfirmed')?'unconfirmed':cases.every(row=>row.status==='pending')?'pending':'partial'
 return value.status===expected
}
