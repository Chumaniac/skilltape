import {isSavedEvidenceId} from './api'
import {receiptName} from './pages/PackageJourney'
const KEY='skilltape.console.verify-preparations.v1',MAX_BYTES=16384,MAX_ITEMS=16,MAX_AGE=86400000
interface Draft {skillId:string;id:string;createdAt:number}
export function verificationPreparation(skillId:string,fresh=false):{id:string;retained:boolean}{
 const now=Date.now();let rows:Draft[]=[]
 try{const text=sessionStorage.getItem(KEY);if(text&&text.length<=MAX_BYTES){const value=JSON.parse(text);if(value?.schema===1&&Array.isArray(value.items)&&value.items.length<=MAX_ITEMS)rows=value.items.filter((row:Draft)=>row&&typeof row.skillId==='string'&&isSavedEvidenceId(row.skillId)&&typeof row.id==='string'&&isSavedEvidenceId(row.id)&&Number.isFinite(row.createdAt)&&now>=row.createdAt&&now-row.createdAt<MAX_AGE)}}catch{/* A blocked session store must not stop read-only command preparation. */}
 const previous=rows.find(row=>row.skillId===skillId);if(previous&&!fresh)return {id:previous.id,retained:true}
 const id=receiptName(skillId)+'-'+crypto.randomUUID().slice(0,8)
 rows=[{skillId,id,createdAt:now},...rows.filter(row=>row.skillId!==skillId)].slice(0,MAX_ITEMS)
 try{sessionStorage.setItem(KEY,JSON.stringify({schema:1,items:rows}));return {id,retained:true}}catch{return {id,retained:false}}
}
