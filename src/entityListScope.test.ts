import {describe,it,expect,vi} from 'vitest';
import {entityListScope} from './entityListScope';
import type {ProfileData} from './exploration';
import type {Work} from './api';
const work=(id:string)=>({id,title:id});
const options=()=>({kind:'person',character:{id:'c1',name:'Actor'},works:[{id:'local',vndb_id:'v2',title:'Local title'}] as Work[],ownedOnly:false,spoilers:false,cancelled:()=>false,progress:vi.fn()});
describe('complete entity list scope',()=>{
 it('collects beyond page one, deduplicates identities and applies owned scope after traversal',async()=>{
  const load=vi.fn(async(page:number)=>({page,more:page===1,works:page===1?[work('v1')]:[work('v1'),work('v2')]}));
  expect(await entityListScope(load,options())).toEqual([{work_key:'vndb:v1',title:'v1'},{work_key:'local:local',title:'Local title'}]);
  expect(await entityListScope(load,{...options(),ownedOnly:true})).toEqual([{work_key:'local:local',title:'Local title'}]);
  expect(load.mock.calls.map(c=>c[0])).toEqual([1,2,1,2]);
 });
 it('continues through explicitly filtered pages and includes later visible works',async()=>{
  const load=vi.fn(async(page:number)=>({page,more:page<3,works:page===3?[work('v2')]:[],...(page<3?{continuation_verified:true}:{})}));
  expect(await entityListScope(load,options())).toEqual([{work_key:'local:local',title:'Local title'}]);expect(load.mock.calls.map(c=>c[0])).toEqual([1,2,3]);
 });
 it('rejects unexplained empty pages and still bounds filtered traversal',async()=>{
  await expect(entityListScope(async page=>({page,more:true,works:[]}),options())).rejects.toThrow('listScopeIncomplete');
  const load=vi.fn(async(page:number)=>({page,more:true,works:[],continuation_verified:true}));await expect(entityListScope(load,options())).rejects.toThrow('listScopeIncomplete');expect(load).toHaveBeenCalledTimes(1000);
 });
 it('honours character spoiler scope using loaded relationship evidence',async()=>{
  const load=async(page:number)=>({page,more:false,works:[work('v1'),work('v2')],character:{id:'c1',name:'C',vns:[{id:'v1',spoiler:0},{id:'v2',spoiler:2}]}});
  expect(await entityListScope(load,{...options(),kind:'character'})).toHaveLength(1);
  expect(await entityListScope(load,{...options(),kind:'character',spoilers:true})).toHaveLength(2);
 });
 it.each([{partial:true},{stale:true},{local_relationships_incomplete:true},{page:7}])('rejects incomplete second page %j',async failure=>{
  await expect(entityListScope(async(page)=>({page,more:page===1,works:[work('v'+page)],...(page===2?failure:{})}),options())).rejects.toThrow('listScopeIncomplete');
 });
 it('rejects repeating pages and a scope over the add cap',async()=>{
  await expect(entityListScope(async page=>({page,more:true,works:[work('v1')]}),options())).rejects.toThrow('listScopeIncomplete');
  await expect(entityListScope(async page=>({page,more:false,works:Array.from({length:10001},(_,i)=>work('v'+(i+1)))}),options())).rejects.toThrow('listScopeTooLarge');
 });
 it('does not return scope or progress after cancelled in-flight request',async()=>{
  let resolve!:(data:ProfileData)=>void,cancelled=false;const o=options();
  const pending=entityListScope(()=>new Promise(r=>resolve=r),{...o,cancelled:()=>cancelled});cancelled=true;
  resolve({page:1,more:false,works:[work('v1')]});await expect(pending).rejects.toThrow('Cancelled');expect(o.progress).not.toHaveBeenCalled();
 });
});
