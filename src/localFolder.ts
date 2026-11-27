import {isDesktop,isLocalDesktop,localFolder,revealInFolder,type Resource,type Root} from './api';

const MAP_PREFIX='galroon.localRootPath.';

/** Join a root folder path and relative resource path, preserving Windows/UNC or POSIX separators. */
export function joinResourcePath(base:string,relative:string):string{
 const cleanBase=base.trim().replace(/[\\/]+$/,'');
 const cleanRel=relative.trim().replace(/^[\\/]+/,'');
 if(!cleanBase)return cleanRel;
 if(!cleanRel)return cleanBase;
 const win=cleanBase.includes('\\')||/^[a-zA-Z]:/.test(cleanBase);
 const sep=win?'\\':'/';
 const parts=cleanRel.split(/[\\/]+/).filter(p=>p&&p!=='.'&&p!=='..');
 return parts.length?`${cleanBase}${sep}${parts.join(sep)}`:cleanBase;
}

export function savedLocalRoot(rootId:string):string{
 try{return localStorage.getItem(MAP_PREFIX+rootId)?.trim()||'';}catch{return '';}
}

export function saveLocalRoot(rootId:string,path:string){
 const trimmed=path.trim();
 try{
  if(trimmed)localStorage.setItem(MAP_PREFIX+rootId,trimmed);
  else localStorage.removeItem(MAP_PREFIX+rootId);
 }catch{/* Mapping remains usable in memory for this session. */}
}

export type ResolvedResourcePath={
 corePath:string;
 openPath:string|null;
 needsMapping:boolean;
 mappedRoot:string;
 rootId:string;
 rootLabel:string;
 rootCorePath:string;
};

export function resolveResourcePath(resource:Resource,roots:Root[]):ResolvedResourcePath{
 const root=roots.find(r=>r.id===resource.root_id);
 const rootCorePath=resource.root_path||root?.path||'';
 const rootLabel=root?.label||'';
 const corePath=joinResourcePath(rootCorePath,resource.path);
 if(isLocalDesktop()){
  return {corePath,openPath:corePath||null,needsMapping:false,mappedRoot:'',rootId:resource.root_id,rootLabel,rootCorePath};
 }
 const mappedRoot=savedLocalRoot(resource.root_id);
 if(isDesktop&&mappedRoot){
  return {corePath,openPath:joinResourcePath(mappedRoot,resource.path),needsMapping:false,mappedRoot,rootId:resource.root_id,rootLabel,rootCorePath};
 }
 return {corePath,openPath:null,needsMapping:isDesktop,mappedRoot:'',rootId:resource.root_id,rootLabel,rootCorePath};
}

/** Pick the best primary resource for one-click folder access from the Work Detail hero. */
export function primaryResourceForWork(workId:string,preferredReleaseId:string|null|undefined,resources:Resource[],roots:Root[]):Resource|null{
 const online=new Set(roots.filter(r=>r.online).map(r=>r.id));
 const mine=resources.filter(r=>r.bindings.some(b=>b.work_id===workId));
 if(!mine.length)return null;
 const score=(r:Resource)=>{
  const binding=r.bindings.find(b=>b.work_id===workId);
  const avail=(online.has(r.root_id)||!roots.length)&&r.missing===0&&r.unverified===0;
  const preferred=preferredReleaseId&&binding?.release_id===preferredReleaseId;
  const main=binding?.role==='main'||binding?.role==='unknown';
  return (avail?8:0)+(preferred?4:0)+(main?2:0)+(r.kind==='folder'?1:0);
 };
 return [...mine].sort((a,b)=>score(b)-score(a))[0];
}

export async function copyText(text:string):Promise<boolean>{
 if(!text||!navigator.clipboard?.writeText)return false;
 try{await navigator.clipboard.writeText(text);return true;}catch{return false;}
}

export {localFolder,revealInFolder};
