import type {Resource,Root} from './api';
export type ResourceState='available'|'offline'|'missing'|'unverified'|'unknown';
export function resourceStates(resource:Resource,roots:Root[]):ResourceState[]{
 const root=roots.find(r=>r.id===resource.root_id);const states:ResourceState[]=[];
 if(!root)states.push('unknown');else if(!root.online)states.push('offline');
 if(resource.missing>0)states.push('missing');if(resource.unverified>0)states.push('unverified');
 if(!resource.files||resource.missing===undefined||resource.unverified===undefined||(resource.unknown||0)>0)states.push('unknown');
 return states.length?[...new Set(states)]:['available'];
}
export function editionResources(workId:string,editionId:string,resources:Resource[],roots:Root[]){
 const members=resources.filter(r=>r.bindings.some(b=>b.work_id===workId&&b.release_id===editionId));
 return {members,available:members.filter(r=>resourceStates(r,roots).includes('available')).length,bytes:members.reduce((n,r)=>n+r.bytes,0),patches:members.filter(r=>r.bindings.some(b=>b.work_id===workId&&b.release_id===editionId&&b.role==='patch')).length};
}

