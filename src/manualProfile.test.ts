import {describe,it,expect,vi} from 'vitest';
import {completeProfileScope,validManualPage,type ManualProfilePage} from './manualProfile';
import type {ProfileData} from './exploration';
const options=()=>({kind:'character',character:{id:'c1',name:'Character',vns:[]},works:[],ownedOnly:false,spoilers:false,cancelled:()=>false,progress:vi.fn()});
const manual=(extra:Partial<ManualProfilePage>={}):ManualProfilePage=>({works:[],partial:false,missing_work_ids:[],next:null,revision:7,manual_credit_subset:true,...extra});
const provider=(page:number):ProfileData=>({page,works:[{id:'v1',title:'Provider',profile_membership:true}],more:false,relationship_revision:7});
describe('complete combined profile scope',()=>{
 it('traverses independent pages, deduplicates works and validates final revision',async()=>{
  const load=vi.fn(async(after:string|null)=>manual(after?{works:[{id:'v2',title:'Manual character appearance',profile_membership:true}]}:{works:[{id:'v1',title:'Fallback',profile_membership:true}],next:'v1'}));
  expect(await completeProfileScope(async(page,revision)=>{expect(revision).toBe(7);return provider(page);},load,options())).toEqual([{work_key:'vndb:v1',title:'Provider'},{work_key:'vndb:v2',title:'Manual character appearance'}]);expect(load.mock.calls.map(v=>v[0])).toEqual([null,'v1',null]);
 });
 it.each([{partial:true},{missing_work_ids:['v9']}])('rejects partial manual metadata %j',async extra=>{await expect(completeProfileScope(async p=>provider(p),async()=>manual(extra),options())).rejects.toThrow('listScopeIncomplete');});
 it('rejects incomplete provider projection and mismatched revision',async()=>{
  for(const extra of [{local_relationships_incomplete:true},{relationship_revision:8}])await expect(completeProfileScope(async p=>({...provider(p),...extra}),async()=>manual(),options())).rejects.toThrow();
 });
 it('rejects revision changes after final page and repeated manual cursors',async()=>{
  let reads=0;await expect(completeProfileScope(async p=>provider(p),async()=>manual({revision:++reads===1?7:8}),options())).rejects.toThrow('profileRelationsChanged');
  await expect(completeProfileScope(async p=>provider(p),async()=>manual({next:'v1'}),options())).rejects.toThrow('listScopeIncomplete');
 });
 it('cancels a manual request without returning members or progress',async()=>{
  let resolve!:(value:ManualProfilePage)=>void,cancelled=false;const o=options();const result=completeProfileScope(async p=>provider(p),()=>new Promise(r=>resolve=r),{...o,cancelled:()=>cancelled});cancelled=true;resolve(manual());await expect(result).rejects.toThrow('Cancelled');expect(o.progress).not.toHaveBeenCalled();
 });
 it('rejects malformed endpoint responses instead of claiming an empty scope',()=>{expect(()=>validManualPage([] as unknown as ManualProfilePage)).toThrow('listScopeIncomplete');});
});
