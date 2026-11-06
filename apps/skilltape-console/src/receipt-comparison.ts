export type Comparability = 'same-recorded-conditions' | 'incomparable' | 'unknown'
interface Identity {id:string;receipt_sha256:string;run_id:string;skill_hash:string;status:'succeeded'|'run_failed'|'cancelled'|'assertion_failed';context_state:'recorded'|'unrecorded'|'invalid'}
interface Outcome {status:string;exit_code:number|null;count:number}
export interface ReceiptDelta {key:string;category:'step'|'assertion'|'policy';kind:string;target:string;phase:string|null;label_truncated:boolean;before:Outcome[];after:Outcome[]}
export interface ReceiptComparison {
 schema:'skilltape.dev/receipt-comparison/v1';before:Identity;after:Identity;comparability:Comparability;condition_findings:string[]
 counts:{new_non_success:number;no_longer_observed_non_success:number;continuing_non_success:number;total_groups:number;outcome_groups_changed:number}
 changes:ReceiptDelta[];change_total:number;changes_truncated:boolean;execution:'not-run';provenance:'not-authenticated';export_approval:'not-granted'
}
const findings=['package_changed','input_changed','executor_changed','platform_changed','limits_changed','context_invalid','conditions_unrecorded']
const record=(v:unknown):v is Record<string,unknown>=>Boolean(v&&typeof v==='object'&&!Array.isArray(v))
const keys=(v:Record<string,unknown>,expected:string[])=>Object.keys(v).length===expected.length&&expected.every(key=>Object.hasOwn(v,key))
const count=(v:unknown,max:number)=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0&&v<=max
const hash=(v:unknown)=>typeof v==='string'&&/^[a-f0-9]{64}$/.test(v)
const text=(v:unknown)=>typeof v==='string'&&v.length>0&&v.length<=482
function identity(v:unknown,id:string):v is Identity {
 return record(v)&&keys(v,['id','receipt_sha256','run_id','skill_hash','status','context_state'])&&v.id===id&&[v.receipt_sha256,v.run_id,v.skill_hash].every(hash)&&['succeeded','run_failed','cancelled','assertion_failed'].includes(String(v.status))&&['recorded','unrecorded','invalid'].includes(String(v.context_state))
}
function outcomes(v:unknown,category:string):boolean {
 if(!Array.isArray(v)||v.length>(category==='step'?1:2))return false
 const allowed=category==='step'?['succeeded','failed','denied','timed_out','cancelled','spawn_failed']:category==='assertion'?['passed','failed']:['allowed','denied']
 return v.every(row=>record(row)&&keys(row,['status','exit_code','count'])&&allowed.includes(String(row.status))&&count(row.count,1000)&&row.count!==0&&(row.exit_code===null||category==='step'&&typeof row.exit_code==='number'&&Number.isInteger(row.exit_code)&&row.exit_code>=-2147483648&&row.exit_code<=2147483647))&&new Set(v.map(row=>String(row.status)+':'+row.exit_code)).size===v.length
}
export function validReceiptComparison(value:unknown,before:string,after:string):value is ReceiptComparison {
 if(!record(value)||!keys(value,['schema','before','after','comparability','condition_findings','counts','changes','change_total','changes_truncated','execution','provenance','export_approval']))return false
 const v=value as unknown as ReceiptComparison
 if(v.schema!=='skilltape.dev/receipt-comparison/v1'||!identity(v.before,before)||!identity(v.after,after)||v.execution!=='not-run'||v.provenance!=='not-authenticated'||v.export_approval!=='not-granted')return false
 if(!Array.isArray(v.condition_findings)||v.condition_findings.length>7||new Set(v.condition_findings).size!==v.condition_findings.length||v.condition_findings.some(code=>!findings.includes(code)))return false
 const knownDifference=v.condition_findings.some(code=>findings.slice(0,5).includes(code))
 const bothRecorded=v.before.context_state==='recorded'&&v.after.context_state==='recorded'
 if(v.comparability!==(knownDifference?'incomparable':bothRecorded?'same-recorded-conditions':'unknown'))return false
 if((v.before.skill_hash!==v.after.skill_hash)!==v.condition_findings.includes('package_changed')||(v.before.context_state==='invalid'||v.after.context_state==='invalid')!==v.condition_findings.includes('context_invalid')||(v.before.context_state==='unrecorded'||v.after.context_state==='unrecorded')!==v.condition_findings.includes('conditions_unrecorded'))return false
 if(!record(v.counts)||!keys(v.counts,['new_non_success','no_longer_observed_non_success','continuing_non_success','total_groups','outcome_groups_changed'])||!Object.values(v.counts).every(n=>count(n,6000)))return false
 if(v.counts.new_non_success+v.counts.continuing_non_success>3000||v.counts.no_longer_observed_non_success+v.counts.continuing_non_success>3000||v.counts.outcome_groups_changed>v.counts.total_groups||!count(v.change_total,6000)||v.change_total>v.counts.total_groups||v.counts.outcome_groups_changed>v.change_total)return false
 if(!Array.isArray(v.changes)||v.changes.length!==Math.min(v.change_total,64)||v.changes_truncated!==(v.change_total>v.changes.length)||new Set(v.changes.map(row=>row.key)).size!==v.changes.length)return false
 return v.changes.every(row=>record(row)&&keys(row,['key','category','kind','target','phase','label_truncated','before','after'])&&hash(row.key)&&['step','assertion','policy'].includes(row.category)&&text(row.kind)&&text(row.target)&&typeof row.label_truncated==='boolean'&&(row.category==='policy'?['before','after'].includes(String(row.phase)):row.phase===null)&&outcomes(row.before,row.category)&&outcomes(row.after,row.category))
}
