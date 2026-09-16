import {beforeEach,describe,expect,it,vi} from 'vitest';
import type {Work} from './api';
const mocked=vi.hoisted(()=>({read:vi.fn()}));vi.mock('./api',()=>({readApi:mocked.read}));
import {createOwnedResolver} from './ownedWorks';
import {completeProfileScope} from './manualProfile';
const owned=(id:string)=>({id:'local-'+id,vndb_id:id,title:'Owned '+id} as Work);
beforeEach(()=>{mocked.read.mockReset();});
describe('bounded ownership lookup',()=>{
 it('looks up every identity in 60-item batches and pins the collection through final verification',async()=>{
  mocked.read.mockImplementation(async(path:string)=>{const q=new URL('http://local'+path).searchParams,ids=JSON.parse(q.get('ids')!) as string[];if(!ids)throw new Error('Unexpected lookup path: '+path);return {items:ids.filter(id=>id!=='v2').map(owned),missing:ids.filter(id=>id==='v2'),revision:12,library_id:'collection-a'};});
  const resolver=createOwnedResolver(new AbortController().signal);const result=await resolver.resolve([...Array.from({length:125},(_,i)=>'v'+i),'v0']);await resolver.verify();
  expect(result).toHaveLength(124);expect(result.at(-1)?.vndb_id).toBe('v124');
  const queries=mocked.read.mock.calls.map(call=>new URL('http://local'+call[0]).searchParams);expect(queries.map(q=>JSON.parse(q.get('ids')!).length)).toEqual([60,60,5,0]);expect(queries[0].has('revision')).toBe(false);for(const q of queries.slice(1)){expect(q.get('revision')).toBe('12');expect(q.get('library_id')).toBe('collection-a');}
 });
 it('rejects missing identities and changed collections instead of reporting false absence',async()=>{
  const resolver=createOwnedResolver(new AbortController().signal);mocked.read.mockResolvedValueOnce({items:[],missing:[],revision:1,library_id:'a'});await expect(resolver.resolve(['v1'])).rejects.toThrow('incomplete');
  mocked.read.mockResolvedValueOnce({items:[owned('v1')],missing:[],revision:1,library_id:'a'}).mockResolvedValueOnce({items:[],missing:[],revision:2,library_id:'a'});await resolver.resolve(['v1']);await expect(resolver.verify()).rejects.toThrow('Collection changed');
 });
 it('resolves ownership for provider and manual pages outside the loaded UI and retains only owned scope',async()=>{
  mocked.read.mockImplementation(async(path:string)=>{const ids=JSON.parse(new URL('http://local'+path).searchParams.get('ids')!) as string[];if(!ids)throw new Error('Unexpected lookup path: '+path);return {items:ids.filter(id=>['v61','v900'].includes(id)).map(owned),missing:ids.filter(id=>!['v61','v900'].includes(id)),revision:7,library_id:'a'};});
  const resolver=createOwnedResolver(new AbortController().signal);const result=await completeProfileScope(async(page)=>({page,works:[{id:page===1?'v1':'v61',title:'Provider',profile_membership:true}],more:page===1,relationship_revision:4}),async()=>({works:[{id:'v900',title:'Manual',profile_membership:true}],partial:false,missing_work_ids:[],next:null,revision:4,manual_credit_subset:true}),{kind:'person',character:{id:'',name:''},works:[],resolveOwned:resolver.resolve,ownedOnly:true,spoilers:false,cancelled:()=>false,progress:()=>{}});await resolver.verify();
  expect(result).toEqual([{work_key:'local:local-v61',title:'Owned v61'},{work_key:'local:local-v900',title:'Owned v900'}]);
 });
});
it('resolves local IDs without confusing provider identities and rejects mismatched results',async()=>{
 mocked.read.mockImplementation(async(path:string)=>{const q=new URL('http://local'+path).searchParams;expect(q.get('local')).toBe('true');const ids=JSON.parse(q.get('ids')!) as string[];return {items:ids.map(id=>({id,vndb_id:'v999',title:id})),missing:[],revision:3,library_id:'a'};});
 const resolver=createOwnedResolver(new AbortController().signal,true);expect(await resolver.resolve(['a','b'])).toHaveLength(2);
 mocked.read.mockResolvedValueOnce({items:[{id:'wrong',vndb_id:'a'}],missing:[],revision:3,library_id:'a'});await expect(resolver.resolve(['a'])).rejects.toThrow('incomplete');
});
