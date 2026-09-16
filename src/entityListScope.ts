import {visibleCharacter,type ProfileData,type Character} from './exploration';
import type {Work} from './api';
import type {ListMember} from './AddToList';
export async function entityListScope(load:(page:number)=>Promise<ProfileData>, options:{kind:string;character:Character;works:Work[];resolveOwned?:(ids:string[])=>Promise<Work[]>;ownedOnly:boolean;spoilers:boolean;cancelled:()=>boolean;progress:(count:number)=>void}):Promise<ListMember[]>{
 const members=new Map<string,ListMember>(),seen=new Set<string>();
 for(let page=1;page<=1000;page++){
  if(options.cancelled())throw new Error('Cancelled');
  const result=await load(page);
  if(options.cancelled())throw new Error('Cancelled');
  if(result.partial||result.stale||result.local_relationships_incomplete||result.page!==page)throw new Error('listScopeIncomplete');
  const ownedWorks=options.resolveOwned?await options.resolveOwned(result.works.map(work=>work.id)):options.works;
  if(options.cancelled())throw new Error('Cancelled');
  let fresh=0;
  for(const work of result.works){
   if(!seen.has(work.id)){seen.add(work.id);fresh++;}
   const owned=ownedWorks.find(w=>w.vndb_id===work.id);
   if(options.ownedOnly&&!owned)continue;
   if(options.kind==='character'&&work.profile_membership!==true&&!visibleCharacter(result.character||options.character,work.id,options.spoilers))continue;
   const key=owned?'local:'+owned.id:'vndb:'+work.id;
   members.set(key,{work_key:key,title:owned?.title||work.title});
   if(members.size>10000)throw new Error('listScopeTooLarge');
  }
  options.progress(members.size);
  if(!result.more)return [...members.values()];
  // An entire provider page may have been removed by local relationship decisions.
  const filteredEmpty=result.works.length===0&&result.continuation_verified===true;
  if(!fresh&&!filteredEmpty)throw new Error('listScopeIncomplete');
 }
 throw new Error('listScopeIncomplete');
}
