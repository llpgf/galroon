import {readApi} from './api';
export type ResourceMember={id:string;relative:string;size:number;availability:string};
export type MemberPage={source:{id:string;root_id:string;title:string;revision:number};items:ResourceMember[];next:string|null;total:number;file_count:number;offset:number;catalog_revision:number;library_id:string};
export type MemberPin=Pick<MemberPage,'catalog_revision'|'library_id'>;
export function memberPagePath(id:string,query:string,pin?:MemberPin,before?:string|null){
 const params=new URLSearchParams({query});if(pin){params.set('catalog_revision',String(pin.catalog_revision));params.set('library_id',pin.library_id);}if(before)params.set('before',before);
 return `/resources/${encodeURIComponent(id)}/members/page?${params}`;
}
/** Retain only IDs across pages. The caller must commit the selection only after this completes. */
export async function collectMatchingMemberIds(id:string,query:string,pin:MemberPin,signal:AbortSignal,onProgress?:(count:number,total:number)=>void):Promise<string[]>{
 const ids=new Set<string>(),cursors=new Set<string>();let before:string|null=null,total:number|null=null,offset=0;
 do{
  signal.throwIfAborted();
  const page:MemberPage=await readApi<MemberPage>(memberPagePath(id,query,pin,before),signal);signal.throwIfAborted();
  if(page.source.id!==id||page.library_id!==pin.library_id||page.catalog_revision!==pin.catalog_revision||page.items.length>60||page.offset!==offset||!Number.isSafeInteger(page.total)||page.total<0||(total!==null&&page.total!==total))throw new Error('Resource files changed. Restart from the first page.');
  total=page.total;
  for(const item of page.items){if(ids.has(item.id))throw new Error('Repeated resource member. Reload its files.');ids.add(item.id);}
  offset+=page.items.length;onProgress?.(offset,total);signal.throwIfAborted();
  before=page.next;if(before){if(page.items.length!==60||cursors.has(before)||offset>=total)throw new Error('Invalid resource member page. Reload its files.');cursors.add(before);}
 }while(before);
 if(ids.size!==total)throw new Error('Resource files changed. Restart from the first page.');
 return [...ids];
}
