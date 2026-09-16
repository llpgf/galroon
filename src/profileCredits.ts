import type {Person,Related,Voice} from './exploration';
const staffKey=(v:Person)=>JSON.stringify([v.id,v.aid??null,v.role??'',v.note??'']);
const voiceKey=(v:Voice)=>JSON.stringify([v.staff.id,v.staff.aid??null,v.character.id,v.note??'']);
function combine<T>(source:T[],manual:T[],key:(value:T)=>string,merge:(a:T,b:T)=>T=(a,b)=>({...a,...b})):T[]{const result=new Map(source.map(v=>[key(v),v]));for(const value of manual){const old=result.get(key(value));result.set(key(value),old?merge(old,value):value);}return [...result.values()];}
/** Manual pages contain only local credits, never replace complete provider work rows with them. */
export function mergeProfileCredits(provider:Related[],manual:Related[]):Related[]{
 const result=new Map(provider.map(v=>[v.id,v]));
 for(const local of manual){const source=result.get(local.id);result.set(local.id,source?{...local,...source,...(source.staff||local.staff?{staff:combine(source.staff||[],local.staff||[],staffKey)}:{}),...(source.va||local.va?{va:combine(source.va||[],local.va||[],voiceKey,(a,b)=>({...a,...b,staff:{...a.staff,...b.staff},character:{...a.character,...b.character}}))}:{})}:local);}
 return [...result.values()];
}

