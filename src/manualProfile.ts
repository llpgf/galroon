import {entityListScope} from './entityListScope';
import type {Related,ProfileData} from './exploration';
import type {ListMember} from './AddToList';
export type ManualProfilePage={works:Related[];partial:boolean;missing_work_ids:string[];next:string|null;revision:number;manual_credit_subset:true};
export function manualProfilePath(id:string,spoilers:boolean,after:string|null=null,revision?:number){
 const query=new URLSearchParams({spoilers:String(spoilers)});if(after)query.set('after',after);if(revision!==undefined)query.set('revision',String(revision));
 return `/entities/${encodeURIComponent(id)}/manual-works?${query}`;
}
export function validManualPage(value:ManualProfilePage,revision?:number):ManualProfilePage{
 if(value.manual_credit_subset!==true||!Array.isArray(value.works)||!Array.isArray(value.missing_work_ids)||!Number.isSafeInteger(value.revision)||value.revision<0||typeof value.partial!=='boolean'||(value.next!==null&&(typeof value.next!=='string'||!/^v[0-9]{1,12}$/.test(value.next))))throw new Error('listScopeIncomplete');
 if(revision!==undefined&&value.revision!==revision)throw new Error('profileRelationsChanged');return value;
}
/** Traverse both independent streams at one relationship revision, never just loaded UI rows. */
export async function completeProfileScope(loadProvider:(page:number,revision:number)=>Promise<ProfileData>,loadManual:(after:string|null,revision?:number)=>Promise<ManualProfilePage>,options:Parameters<typeof entityListScope>[1]):Promise<ListMember[]>{
 const check=()=>{if(options.cancelled())throw new Error('Cancelled');};check();
 let manual=validManualPage(await loadManual(null));check();const revision=manual.revision;
 const provider=await entityListScope(async page=>{const data=await loadProvider(page,revision);if(data.relationship_revision!==revision)throw new Error('profileRelationsChanged');return data;},options);check();
 const members=new Map(provider.map(value=>[value.work_key,value]));const cursors=new Set<string>();
 for(let pages=0;pages<1000;pages++){
  check();if(manual.partial||manual.missing_work_ids.length)throw new Error('listScopeIncomplete');
  const ownedWorks=options.resolveOwned?await options.resolveOwned(manual.works.map(work=>work.id)):options.works;check();
  for(const work of manual.works){
   if(work.profile_membership!==true)throw new Error('listScopeIncomplete');
   const owned=ownedWorks.find(w=>w.vndb_id===work.id);if(options.ownedOnly&&!owned)continue;
   const key=owned?'local:'+owned.id:'vndb:'+work.id;if(!members.has(key))members.set(key,{work_key:key,title:owned?.title||work.title});
   if(members.size>10000)throw new Error('listScopeTooLarge');
  }
  options.progress(members.size);
  if(!manual.next){check();validManualPage(await loadManual(null,revision),revision);check();return [...members.values()];}
  if(cursors.has(manual.next))throw new Error('listScopeIncomplete');cursors.add(manual.next);
  manual=validManualPage(await loadManual(manual.next,revision),revision);
 }
 throw new Error('listScopeIncomplete');
}
