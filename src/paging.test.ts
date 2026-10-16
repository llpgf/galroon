import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
// Like WebLogin's tests, exercise hooks without adding a browser dependency.
// Preserve state, dependency comparisons and effect cleanup across explicit renders.
const hooks=vi.hoisted(()=>({slots:[] as any[],cursor:0,effects:[] as (()=>void)[]}));
vi.mock('react',async original=>({...await original<typeof import('react')>(),
 useState:(initial:unknown)=>{const i=hooks.cursor++;if(!(i in hooks.slots))hooks.slots[i]=typeof initial==='function'?initial():initial;return [hooks.slots[i],(value:unknown)=>{hooks.slots[i]=typeof value==='function'?value(hooks.slots[i]):value;}];},
 useRef:(initial:unknown)=>{const i=hooks.cursor++;return hooks.slots[i]??= {current:initial};},
 useMemo:(fn:()=>unknown,deps:unknown[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||deps.some((d,j)=>!Object.is(d,old.deps[j])))hooks.slots[i]={value:fn(),deps};return hooks.slots[i].value;},
 useEffect:(fn:()=>void|(()=>void),deps:unknown[])=>{const i=hooks.cursor++,old=hooks.slots[i];if(!old||deps.some((d,j)=>!Object.is(d,old.deps[j])))hooks.effects.push(()=>{old?.cleanup?.();hooks.slots[i]={deps,cleanup:fn()};});},
}));
const mock=vi.hoisted(()=>({readApi:vi.fn()}));vi.mock('./api',()=>mock);
import {useCollectionPage,type CollectionPage,type CollectionQuery} from './useCollectionPage';
import {useResourcePage,type ResourcePage} from './useResourcePage';
import {useIssuePages} from './useIssuePages';
import {emptyFilters} from './library';
const query:CollectionQuery={filters:emptyFilters,query:'',status:'all',sort:'added',titleMode:'display',locale:'en'};
function render<T>(fn:()=>T):T{hooks.cursor=0;const result=fn();const effects=hooks.effects.splice(0);effects.forEach(effect=>effect());return result;}
async function settle<T>(fn:()=>T){await vi.runAllTimersAsync();render(fn);await vi.runAllTimersAsync();return render(fn);}
function page(ids:string[],next:string|null='page-2',total=120){return {items:ids.map(id=>({id,status:'none',favorite:false})),next,total,filtered:total,offset:0,revision:1,library_id:'library',view_token:'view',options:{studio:[],year:[],language:[],tag:[]},tag_counts:{},status_counts:{none:total},custom_tags:[]} as unknown as CollectionPage&ResourcePage;}
beforeEach(()=>{hooks.slots=[];hooks.effects=[];hooks.cursor=0;vi.resetAllMocks();vi.useFakeTimers();});
afterEach(()=>{hooks.slots.forEach(slot=>slot?.cleanup?.());vi.useRealTimers();});

for(const kind of ['collection','resources'] as const)describe(kind+' paging refresh',()=>{
 let generation:number,request:CollectionQuery;
 const usePage=()=>kind==='collection'?useCollectionPage(request,generation):useResourcePage(request.query,request.status,generation);
 beforeEach(()=>{generation=0;request=query;});
 it('keeps rows, counts and the current cursor until the refreshed window arrives',async()=>{
  mock.readApi.mockResolvedValueOnce(page(['first']));render(usePage);let state=await settle(usePage);
  mock.readApi.mockResolvedValueOnce(page(['kept','removed'],'page-3'));state.next();render(usePage);state=await settle(usePage);
  let resolve!:(value:unknown)=>void;mock.readApi.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));generation++;
  state=render(usePage);expect(state.pageIndex).toBe(1);expect(state.loading).toBe(false);expect(state.data?.items.map(w=>w.id)).toEqual(['kept','removed']);
  await vi.runAllTimersAsync();expect(mock.readApi.mock.lastCall?.[0]).toContain('before=page-2');
  const fresh=page(['kept','added'],'new-page-3',121);fresh.items[0].favorite=true;fresh.status_counts={playing:1,none:120};resolve(fresh);
  state=await settle(usePage);expect(state.pageIndex).toBe(1);expect(state.data).toBe(fresh);expect(state.restarted).toBe(false);
  mock.readApi.mockResolvedValueOnce(page(['third'],null));state.next();render(usePage);await settle(usePage);expect(mock.readApi.mock.lastCall?.[0]).toContain('before=new-page-3');
 });
 it('refreshes on page one and on manual retry without blanking rows or losing the cursor trail',async()=>{
  mock.readApi.mockResolvedValue(page(['one']));render(usePage);let state=await settle(usePage);
  generation++;expect(render(usePage).data?.items[0].id).toBe('one');state=await settle(usePage);
  mock.readApi.mockResolvedValue(page(['two'],null));state.next();render(usePage);state=await settle(usePage);
  state.reload();state=render(usePage);expect(state.pageIndex).toBe(1);expect(state.data?.items[0].id).toBe('two');state=await settle(usePage);
  state.previous();render(usePage);await settle(usePage);expect(mock.readApi.mock.lastCall?.[0]).not.toContain('before=');
 });
 it('resets on a changed query and ignores late responses from the old view',async()=>{
  mock.readApi.mockResolvedValueOnce(page(['one']));render(usePage);let state=await settle(usePage);
  mock.readApi.mockResolvedValueOnce(page(['two'],null));state.next();render(usePage);await settle(usePage);
  let resolve!:(value:unknown)=>void;mock.readApi.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));generation++;render(usePage);await vi.runAllTimersAsync();
  const signal=mock.readApi.mock.lastCall?.[1] as AbortSignal;request={...query,query:'new search'};mock.readApi.mockResolvedValueOnce(page(['new result'],null));state=render(usePage);
  expect(state.pageIndex).toBe(0);expect(state.data).toBeNull();expect(signal.aborted).toBe(true);await settle(usePage);resolve(page(['obsolete'],null));state=await settle(usePage);
  expect(state.data?.items[0].id).toBe('new result');expect(mock.readApi.mock.lastCall?.[0]).not.toContain('before=');
  request=query;state=render(usePage);expect(state.pageIndex).toBe(0);expect(state.data).toBeNull();
 });
 it('restarts only on a rejected stale cursor and keeps the explanation',async()=>{
  mock.readApi.mockResolvedValueOnce(page(['one']));render(usePage);let state=await settle(usePage);
  mock.readApi.mockResolvedValueOnce(page(['two'],null));state.next();render(usePage);await settle(usePage);
  const message=kind==='collection'?'Collection or filters changed. Reload before continuing.':'Resources changed. Restart from the first page.';
  mock.readApi.mockRejectedValueOnce(new Error(message)).mockResolvedValueOnce(page(['fresh']));generation++;render(usePage);state=await settle(usePage);
  expect(state.pageIndex).toBe(0);expect(state.restarted).toBe(true);expect(state.data?.items[0].id).toBe('fresh');expect(mock.readApi.mock.lastCall?.[0]).not.toContain('before=');
  generation++;mock.readApi.mockResolvedValueOnce(page(['newer']));render(usePage);state=await settle(usePage);expect(state.restarted).toBe(true);
  request={...query,status:'playing'};state=render(usePage);expect(state.restarted).toBe(false);
 });
 it('keeps the stale-cursor fallback when paging without a notification',async()=>{
  mock.readApi.mockResolvedValueOnce(page(['one']));render(usePage);let state=await settle(usePage);
  const message=kind==='collection'?'Collection or filters changed. Reload before continuing.':'Resources changed. Restart from the first page.';
  mock.readApi.mockRejectedValueOnce(new Error(message)).mockResolvedValueOnce(page(['fresh']));state.next();render(usePage);state=await settle(usePage);
  expect(state.pageIndex).toBe(0);expect(state.restarted).toBe(true);expect(state.data?.items[0].id).toBe('fresh');
 });
 it('keeps the current rows on transient refresh errors and retries in place',async()=>{
  mock.readApi.mockResolvedValueOnce(page(['one']));render(usePage);let state=await settle(usePage);
  mock.readApi.mockResolvedValueOnce(page(['two'],null));state.next();render(usePage);await settle(usePage);
  mock.readApi.mockRejectedValueOnce(new Error('offline'));generation++;render(usePage);state=await settle(usePage);
  expect(state.pageIndex).toBe(1);expect(state.data?.items[0].id).toBe('two');expect(state.error).toBe('offline');expect(state.restarted).toBe(false);
  mock.readApi.mockResolvedValueOnce(page(['updated'],null));state.reload();render(usePage);state=await settle(usePage);expect(state.error).toBe('');expect(state.data?.items[0].id).toBe('updated');
 });
 it('ignores superseded refreshes even when the transport completes after cancellation',async()=>{
  mock.readApi.mockResolvedValueOnce(page(['one']));render(usePage);await settle(usePage);
  let resolve!:(value:unknown)=>void;mock.readApi.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));generation++;render(usePage);await vi.runAllTimersAsync();
  const signal=mock.readApi.mock.lastCall?.[1] as AbortSignal;mock.readApi.mockResolvedValueOnce(page(['latest']));generation++;render(usePage);await settle(usePage);
  expect(signal.aborted).toBe(true);resolve(page(['obsolete']));expect((await settle(usePage)).data?.items[0].id).toBe('latest');
 });
});

it.each(['filters','sort','status'] as const)('collection resets when %s changes',async field=>{
 let request=query;const usePage=()=>useCollectionPage(request,0);
 mock.readApi.mockResolvedValueOnce(page(['one']));render(usePage);let state=await settle(usePage);
 mock.readApi.mockResolvedValueOnce(page(['two'],null));state.next();render(usePage);await settle(usePage);
 request={...request,...(field==='filters'?{filters:{...emptyFilters,favorites:true}}:field==='sort'?{sort:'title'}:{status:'playing'})};
 state=render(usePage);expect(state.pageIndex).toBe(0);expect(state.data).toBeNull();
});

describe('issues loaded window',()=>{
 const issuePage=(ids:number[],next:number|null)=>({items:ids.map(id=>({id,title:'Issue '+id})),next});
 it('refreshes every loaded page atomically with fresh cursors, adding and removing issues',async()=>{
  let generation=0;const usePage=()=>useIssuePages('open',generation);
  mock.readApi.mockResolvedValueOnce(issuePage([9,8],8));render(usePage);let state=await settle(usePage);
  mock.readApi.mockResolvedValueOnce(issuePage([7,6],6));await state.more();state=render(usePage);
  expect(state.items.map(i=>i.id)).toEqual([9,8,7,6]);
  let resolve!:(value:unknown)=>void;mock.readApi.mockResolvedValueOnce(issuePage([10,9],9)).mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));
  generation++;render(usePage);await vi.runAllTimersAsync();state=render(usePage);expect(state.items.map(i=>i.id)).toEqual([9,8,7,6]);expect(mock.readApi.mock.lastCall?.[0]).toBe('/issues?state=open&before=9');
  resolve(issuePage([7,6],6));state=await settle(usePage);expect(state.items.map(i=>i.id)).toEqual([10,9,7,6]);expect(state.next).toBe(6);
  mock.readApi.mockResolvedValueOnce(issuePage([10],null));generation++;render(usePage);state=await settle(usePage);expect(state.items.map(i=>i.id)).toEqual([10]);expect(state.next).toBeNull();
 });
 it('keeps all rows if a later refresh page fails, and drops them only for a different filter',async()=>{
  let filter='open',generation=0;const usePage=()=>useIssuePages(filter,generation);
  mock.readApi.mockResolvedValueOnce(issuePage([9],9));render(usePage);let state=await settle(usePage);
  mock.readApi.mockResolvedValueOnce(issuePage([8],8));await state.more();
  mock.readApi.mockResolvedValueOnce(issuePage([10],10)).mockRejectedValueOnce(new Error('offline'));generation++;render(usePage);state=await settle(usePage);
  expect(state.items.map(i=>i.id)).toEqual([9,8]);expect(state.error).toBe('offline');
  mock.readApi.mockResolvedValueOnce(issuePage([5],null));filter='resolved';state=render(usePage);expect(state.items).toEqual([]);state=await settle(usePage);expect(state.items.map(i=>i.id)).toEqual([5]);expect(mock.readApi.mock.lastCall?.[0]).toBe('/issues?state=resolved');
 });
 it('cancels an older loaded-window refresh when a new notification arrives',async()=>{
  let generation=0;const usePage=()=>useIssuePages('open',generation);
  mock.readApi.mockResolvedValueOnce(issuePage([9],9));render(usePage);await settle(usePage);
  let resolve!:(value:unknown)=>void;mock.readApi.mockImplementationOnce(()=>new Promise(r=>{resolve=r;}));generation++;render(usePage);
  mock.readApi.mockResolvedValueOnce(issuePage([10],null));generation++;render(usePage);await settle(usePage);
  resolve(issuePage([8],null));expect((await settle(usePage)).items.map(i=>i.id)).toEqual([10]);
 });
});
