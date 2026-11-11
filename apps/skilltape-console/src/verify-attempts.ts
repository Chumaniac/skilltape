import {isRecordedReceiptStatus,type RecordedReceiptStatus} from './receipt-comparison'

export interface VerifyAttempt {
 id:string;started_at_ms:number;finished_at_ms:number|null
 outcome:'receipt_produced'|'preparation_failed'|'unconfirmed'
 receipt:{receipt_sha256:string;run_id:string;skill_hash:string;input_set_sha256:string;status:RecordedReceiptStatus}|null
 record_integrity:'start-only'|'metadata-bound';read_only:true;provenance:'not-authenticated';saved_receipt_verified:false
}
const object=(v:unknown):v is Record<string,unknown>=>Boolean(v&&typeof v==='object'&&!Array.isArray(v))
const keys=(v:Record<string,unknown>,names:string[])=>Object.keys(v).length===names.length&&names.every(name=>Object.hasOwn(v,name))
const hash=(v:unknown)=>typeof v==='string'&&/^[a-f0-9]{64}$/.test(v)
const time=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>0&&v<=253402300799999
export function validVerifyAttempt(value:unknown):value is VerifyAttempt {
 if(!object(value)||!keys(value,['id','started_at_ms','finished_at_ms','outcome','receipt','record_integrity','read_only','provenance','saved_receipt_verified']))return false
 const v=value
 if(!hash(v.id)||!time(v.started_at_ms)||v.read_only!==true||v.provenance!=='not-authenticated'||v.saved_receipt_verified!==false)return false
 if(v.record_integrity==='start-only')return v.finished_at_ms===null&&v.outcome==='unconfirmed'&&v.receipt===null
 if(v.record_integrity!=='metadata-bound'||!time(v.finished_at_ms)||v.finished_at_ms<v.started_at_ms)return false
 if(v.outcome==='preparation_failed'||v.outcome==='unconfirmed')return v.receipt===null
 if(v.outcome!=='receipt_produced'||!object(v.receipt)||!keys(v.receipt,['receipt_sha256','run_id','skill_hash','input_set_sha256','status']))return false
 const r=v.receipt
 return [r.receipt_sha256,r.run_id,r.skill_hash,r.input_set_sha256].every(hash)&&isRecordedReceiptStatus(r.status)
}
