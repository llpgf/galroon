import {readApi,type Work} from './api';
export type OwnedLookup={items:Work[];missing:string[];revision:number;library_id:string};
/** One list preparation keeps all provider/local decisions at the same catalog revision. */
export function createOwnedResolver(signal:AbortSignal,local=false){
 let snapshot:{revision:number;library_id:string}|undefined;
 async function resolve(ids:string[]):Promise<Work[]>{
  const unique=[...new Set(ids)];const result:Work[]=[];
  for(let start=0;start<Math.max(1,unique.length);start+=60){
   const batch=unique.slice(start,start+60),query=new URLSearchParams({ids:JSON.stringify(batch)});if(local)query.set('local','true');
   if(snapshot){query.set('revision',String(snapshot.revision));query.set('library_id',snapshot.library_id);}
   const page=await readApi<OwnedLookup>('/works/lookup?'+query,signal);
   if(!Number.isSafeInteger(page.revision)||page.revision<1||!page.library_id||!Array.isArray(page.items)||!Array.isArray(page.missing))throw new Error('Collection lookup is incomplete. Retry before continuing.');
   if(snapshot&&(page.revision!==snapshot.revision||page.library_id!==snapshot.library_id))throw new Error('Collection changed. Reload before continuing.');
   const returned=[...page.items.map(w=>local?w.id:w.vndb_id),...page.missing];
   if(returned.length!==batch.length||new Set(returned).size!==batch.length||returned.some(id=>!id||!batch.includes(id)))throw new Error('Collection lookup is incomplete. Retry before continuing.');
   snapshot={revision:page.revision,library_id:page.library_id};result.push(...page.items);
  }
  return result;
 }
 return {resolve,verify:async()=>{await resolve([]);}};
}
