vi.mock('./clientDiagnostics',()=>({reportClientError:vi.fn()}));
import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
const native=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke:native.invoke,isTauri:()=>true}));
import {api,computeRules,previewRules,restoreCore,setSession,SessionChangedError,sessionChanging,ConnectionLostError,UncertainOperationError,connectionSnapshot,connect,stopCore,CollectionChangedError,CompatibilityError,collectionStorageKey,selectConnection} from './api';
function deferred<T>(){let resolve!:(value:T)=>void,reject!:(error:unknown)=>void;const promise=new Promise<T>((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};}
const first={url:'http://127.0.0.1:41001',token:'synthetic-first',library_id:'library-a',device_id:'device-a',api_min:1,api_max:1},second={url:'http://127.0.0.1:41002',token:'synthetic-second',library_id:'library-a',device_id:'device-a',api_min:1,api_max:1};
describe('collection connection transitions',()=>{
 it('endpoint repair blocks writes, retains context on failure and never reloads a rejected address',async()=>{
  const {updateConnectionEndpoint}=await import('./api');const pending=deferred<void>();native.invoke.mockReturnValue(pending.promise);const reload=vi.fn();vi.stubGlobal('window',{location:{reload}});
  const repair=updateConnectionEndpoint('saved-core','http://127.0.0.1:42000');const checked=expect(repair).rejects.toBe('Identity mismatch');
  await expect(api('/works',{})).rejects.toBeInstanceOf(SessionChangedError);pending.reject('Identity mismatch');await checked;
  expect(native.invoke).toHaveBeenCalledWith('update_connection_endpoint',{id:'saved-core',url:'http://127.0.0.1:42000'});expect(reload).not.toHaveBeenCalled();expect(sessionChanging()).toBe(false);expect(collectionStorageKey('history')).toContain(first.library_id);
 });
 it('failed explicit selection retains the current collection without replaying requests',async()=>{
  const selection=deferred<void>();native.invoke.mockReturnValue(selection.promise);const reload=vi.fn();vi.stubGlobal('window',{location:{reload}});
  const switching=selectConnection('saved-core');const checked=expect(switching).rejects.toBe('Unavailable');
  await expect(api('/works',{})).rejects.toBeInstanceOf(SessionChangedError);selection.reject('Unavailable');await checked;
  expect(reload).not.toHaveBeenCalled();expect(sessionChanging()).toBe(false);expect(collectionStorageKey('history')).toContain(first.library_id);
 });
 it('explicit selection reloads only after the native selection succeeds',async()=>{
  vi.resetModules();const fresh=await import('./api');fresh.setSession(first);const pending=deferred<void>();native.invoke.mockReturnValue(pending.promise);const reload=vi.fn();vi.stubGlobal('window',{location:{reload}});
  const selecting=fresh.selectConnection(null);expect(reload).not.toHaveBeenCalled();pending.resolve();await selecting;expect(reload).toHaveBeenCalledTimes(1);expect(native.invoke).toHaveBeenCalledWith('select_connection',{id:null});
 });
 afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();});
 beforeEach(()=>{vi.useFakeTimers();vi.unstubAllGlobals();native.invoke.mockReset();setSession(first);});
 it('discards a late success from the previous Core and uses the restored session',async()=>{
  const pending=deferred<Response>(),restored=deferred<typeof second>();const fetch=vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue(new Response('{"collection":"new"}'));vi.stubGlobal('fetch',fetch);native.invoke.mockReturnValue(restored.promise);
  const old=api('/works');const oldCheck=expect(old).rejects.toBeInstanceOf(SessionChangedError);const switching=restoreCore('synthetic-backup','reviewed-digest');expect(sessionChanging()).toBe(true);await expect(api('/jobs')).rejects.toBeInstanceOf(SessionChangedError);expect(fetch).toHaveBeenCalledTimes(1);
  restored.resolve(second);await switching;pending.resolve(new Response('{"collection":"old"}'));await oldCheck;expect(sessionChanging()).toBe(false);expect(await api('/works')).toEqual({collection:'new'});expect(fetch.mock.calls[1][0]).toBe(`${second.url}/api/works`);
 });
 it('suppresses an old network failure while preserving real failures on the new Core',async()=>{
  const pending=deferred<Response>(),restored=deferred<typeof second>();vi.stubGlobal('fetch',vi.fn().mockReturnValueOnce(pending.promise).mockRejectedValue(new Error('new connection failed')));native.invoke.mockReturnValue(restored.promise);
  const old=api('/jobs');const oldCheck=expect(old).rejects.toBeInstanceOf(SessionChangedError);const switching=restoreCore('synthetic-backup','digest');pending.reject(new TypeError('Failed to fetch'));await oldCheck;restored.resolve(second);await switching;await expect(api('/jobs')).rejects.toBeInstanceOf(ConnectionLostError);
 });
 it('reconnects the authoritative current Core after a restore failure',async()=>{
  native.invoke.mockRejectedValueOnce(new Error('restore failed')).mockResolvedValueOnce(second);await expect(restoreCore('bad-backup','digest')).rejects.toThrow('restore failed');expect(sessionChanging()).toBe(false);const fetch=vi.fn().mockResolvedValue(new Response('[]'));vi.stubGlobal('fetch',fetch);await api('/works');expect(fetch.mock.calls[0][0]).toBe(`${second.url}/api/works`);expect(native.invoke.mock.calls[1][0]).toBe('core_session');
 });
 it('coalesces automatic and manual reconnection and uses fresh credentials without replay',async()=>{
  const next=deferred<typeof second>();native.invoke.mockReturnValue(next.promise);const fetch=vi.fn().mockRejectedValueOnce(new TypeError('offline')).mockResolvedValue(new Response('[]'));vi.stubGlobal('fetch',fetch);
  await expect(api('/jobs')).rejects.toBeInstanceOf(ConnectionLostError);expect(connectionSnapshot().phase).toBe('offline');await expect(api('/works')).rejects.toBeInstanceOf(ConnectionLostError);expect(fetch).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(500);const manual=connect();expect(native.invoke).toHaveBeenCalledTimes(1);expect(connectionSnapshot().phase).toBe('reconnecting');next.resolve(second);await manual;expect(connectionSnapshot().phase).toBe('connected');expect(fetch).toHaveBeenCalledTimes(1);
  await api('/works');expect(fetch.mock.calls[1][0]).toBe(second.url+'/api/works');expect(fetch.mock.calls[1][1].headers.Authorization).toBe('Bearer '+second.token);
 });
 it('never replays a mutation whose outcome was lost',async()=>{
  native.invoke.mockResolvedValue(second);const fetch=vi.fn().mockRejectedValue(new TypeError('lost response'));vi.stubGlobal('fetch',fetch);
  await expect(api('/plans/p/execute',{})).rejects.toBeInstanceOf(UncertainOperationError);await vi.advanceTimersByTimeAsync(500);expect(connectionSnapshot().phase).toBe('connected');expect(fetch).toHaveBeenCalledTimes(1);expect(fetch.mock.calls[0][1].method).toBe('POST');
 });
 it('reports an uncertain mutation when a parallel read changes the connection epoch',async()=>{
  const write=deferred<Response>();vi.stubGlobal('fetch',vi.fn().mockReturnValueOnce(write.promise).mockRejectedValue(new TypeError('offline')));
  const submitted=api('/works/w',{revision:1,notes:'draft'});const checked=expect(submitted).rejects.toBeInstanceOf(UncertainOperationError);await expect(api('/jobs')).rejects.toBeInstanceOf(ConnectionLostError);write.resolve(new Response('{"ok":true}'));await checked;
 });
 it('backs off repeated IPC failures rather than launching on every poll',async()=>{
  native.invoke.mockRejectedValue(new Error('Core unavailable'));vi.stubGlobal('fetch',vi.fn().mockRejectedValue(new TypeError('offline')));await expect(api('/jobs')).rejects.toBeInstanceOf(ConnectionLostError);
  await vi.advanceTimersByTimeAsync(499);expect(native.invoke).not.toHaveBeenCalled();await vi.advanceTimersByTimeAsync(1);expect(native.invoke).toHaveBeenCalledTimes(1);await vi.advanceTimersByTimeAsync(999);expect(native.invoke).toHaveBeenCalledTimes(1);await vi.advanceTimersByTimeAsync(1);expect(native.invoke).toHaveBeenCalledTimes(2);await vi.advanceTimersByTimeAsync(1999);expect(native.invoke).toHaveBeenCalledTimes(2);await vi.advanceTimersByTimeAsync(1);expect(native.invoke).toHaveBeenCalledTimes(3);expect(connectionSnapshot()).toEqual({phase:'offline',error:'Core unavailable'});
 });
 it('reconnects after 401 but preserves ordinary application errors',async()=>{
  native.invoke.mockResolvedValue(second);const fetch=vi.fn().mockResolvedValueOnce(new Response('{"error":"Conflict"}',{status:400})).mockResolvedValueOnce(new Response('{"error":"Unauthorized"}',{status:401}));vi.stubGlobal('fetch',fetch);
  await expect(api('/plans/p/execute',{})).rejects.toThrow('Conflict');expect(connectionSnapshot().phase).toBe('connected');expect(vi.getTimerCount()).toBe(0);await expect(api('/jobs')).rejects.toBeInstanceOf(ConnectionLostError);await vi.advanceTimersByTimeAsync(500);expect(native.invoke).toHaveBeenCalledOnce();expect(connectionSnapshot().phase).toBe('connected');
 });
 it('cancels preview without disconnect or uncertain write classification',async()=>{
  const fetch=vi.fn().mockImplementation((_url,options)=>new Promise((_resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new DOMException('Cancelled','AbortError')))));vi.stubGlobal('fetch',fetch);
  const controller=new AbortController();const pending=previewRules({},controller.signal);const checked=expect(pending).rejects.toMatchObject({name:'AbortError'});controller.abort();await checked;
  expect(connectionSnapshot().phase).toBe('connected');expect(native.invoke).not.toHaveBeenCalled();expect(vi.getTimerCount()).toBe(0);
  await expect(previewRules({},controller.signal)).rejects.toMatchObject({name:'AbortError'});expect(fetch).toHaveBeenCalledOnce();
 });
 it('cancels manual computation without reconnecting and keeps session drift uncertain',async()=>{
  const fetch=vi.fn().mockImplementation((_url,options)=>new Promise((_resolve,reject)=>options.signal.addEventListener('abort',()=>reject(new DOMException('Cancelled','AbortError')))));vi.stubGlobal('fetch',fetch);
  const controller=new AbortController();const pending=computeRules('s',{revision:1,request_id:'same-receipt'},controller.signal);const checked=expect(pending).rejects.toMatchObject({name:'AbortError'});controller.abort();await checked;
  expect(connectionSnapshot().phase).toBe('connected');expect(native.invoke).not.toHaveBeenCalled();expect(vi.getTimerCount()).toBe(0);
  const later=deferred<Response>();fetch.mockReturnValueOnce(later.promise);const next=computeRules('s',{revision:1,request_id:'same-receipt'},new AbortController().signal);const rejected=expect(next).rejects.toBeInstanceOf(UncertainOperationError);setSession(second);later.resolve(new Response('{}'));await rejected;
 });
 it('discards preview from a previous session as a read',async()=>{
  const pending=deferred<Response>();vi.stubGlobal('fetch',vi.fn().mockReturnValue(pending.promise));
  const preview=previewRules({},new AbortController().signal);const checked=expect(preview).rejects.toBeInstanceOf(SessionChangedError);setSession(second);pending.resolve(new Response('{}'));await checked;
 });
 it('bounds hanging reads and leaves long writes under the caller control',async()=>{
  const fetch=vi.fn().mockImplementation((_url,options)=>new Promise((_resolve,reject)=>options.signal?.addEventListener('abort',()=>reject(new DOMException('Read timed out','AbortError')))));vi.stubGlobal('fetch',fetch);
  const read=api('/jobs');const checked=expect(read).rejects.toBeInstanceOf(ConnectionLostError);await vi.advanceTimersByTimeAsync(15000);await checked;expect(connectionSnapshot().phase).toBe('offline');
  setSession(first);const write=deferred<Response>();fetch.mockReturnValueOnce(write.promise);const operation=api('/plans/p/execute',{});await vi.advanceTimersByTimeAsync(16000);expect(fetch.mock.calls[1][1].signal).toBeUndefined();expect(native.invoke).not.toHaveBeenCalled();write.resolve(new Response('{"ok":true}'));await expect(operation).resolves.toEqual({ok:true});
 });
 it('does not restart Core after an explicit stop request, including failed stop confirmation',async()=>{
  native.invoke.mockRejectedValueOnce(new Error('stop confirmation unavailable')).mockResolvedValueOnce(second);vi.stubGlobal('fetch',vi.fn().mockRejectedValue(new TypeError('offline')));await expect(api('/jobs')).rejects.toBeInstanceOf(ConnectionLostError);await expect(stopCore()).rejects.toThrow('stop confirmation');await vi.advanceTimersByTimeAsync(60000);expect(native.invoke).toHaveBeenCalledTimes(1);expect(connectionSnapshot().phase).toBe('stopped');await connect();expect(connectionSnapshot().phase).toBe('connected');expect(native.invoke.mock.calls[1][0]).toBe('core_session');
 });
 it('pins library and device IDs on writes and blocks a same-named replacement during reconnect',async()=>{
  const fetch=vi.fn().mockRejectedValue(new TypeError('offline'));vi.stubGlobal('fetch',fetch);native.invoke.mockResolvedValue({...second,library_id:'library-b'});
  await expect(api('/works/w',{notes:'private draft'})).rejects.toBeInstanceOf(UncertainOperationError);
  expect(fetch.mock.calls[0][1].headers['X-Galroon-Library']).toBe('library-a');expect(fetch.mock.calls[0][1].headers['X-Galroon-Device']).toBe('device-a');
  await vi.advanceTimersByTimeAsync(500);expect(connectionSnapshot().phase).toBe('collection_changed');await vi.advanceTimersByTimeAsync(60000);expect(native.invoke).toHaveBeenCalledTimes(1);
  await expect(connect()).rejects.toBeInstanceOf(CollectionChangedError);expect(native.invoke).toHaveBeenCalledTimes(1);expect(collectionStorageKey('destination')).toContain('library-a.device-a');
 });
 it('rejects a changed device and incompatible API without looping',async()=>{
  native.invoke.mockResolvedValue({...second,device_id:'device-b'});await expect(connect()).rejects.toBeInstanceOf(CollectionChangedError);setSession(first);
  native.invoke.mockResolvedValue({...second,api_min:2,api_max:3});await expect(connect()).rejects.toBeInstanceOf(CompatibilityError);await vi.advanceTimersByTimeAsync(60000);expect(connectionSnapshot().phase).toBe('incompatible');expect(native.invoke).toHaveBeenCalledTimes(2);
 });
 it('blocks response identity drift and explicit server rejection without adopting data',async()=>{
  vi.stubGlobal('fetch',vi.fn().mockResolvedValueOnce(new Response('{"other":"data"}',{headers:{'x-galroon-library':'library-b'}})).mockResolvedValueOnce(new Response('{"code":"collection_changed"}',{status:409})));
  await expect(api('/works')).rejects.toBeInstanceOf(CollectionChangedError);expect(connectionSnapshot().phase).toBe('collection_changed');
  setSession(first);await expect(api('/works/w',{notes:'draft'})).rejects.toBeInstanceOf(CollectionChangedError);expect(collectionSnapshotKey()).toBe('galroon.library-a.device-a.test');
  function collectionSnapshotKey(){return collectionStorageKey('test');}
 });
 it('isolates destination preferences by collection and device, including explicit restore',async()=>{
  const key=collectionStorageKey('destination');native.invoke.mockResolvedValue({...second,library_id:'restored-library'});await restoreCore('reviewed-backup','digest');
  expect(collectionStorageKey('destination')).not.toBe(key);expect(collectionStorageKey('destination')).toContain('restored-library');expect(connectionSnapshot().phase).toBe('connected');
 });

});

describe('collection paging capability',()=>{
 it('rejects an older Core with an actionable compatibility state',async()=>{vi.resetModules();const fresh=await import('./api');expect(()=>fresh.requireCollectionPaging({capabilities:{}} as any)).toThrow('Update Core');expect(fresh.connectionSnapshot().phase).toBe('incompatible');});
 it('rejects a paging-only Core that lacks scoped work reads',async()=>{vi.resetModules();const fresh=await import('./api');expect(()=>fresh.requireCollectionPaging({capabilities:{collection_paging:true}} as any)).toThrow('Update Core');});
 it('rejects a Core lacking bounded edition membership editing',async()=>{vi.resetModules();const fresh=await import('./api');expect(()=>fresh.requireCollectionPaging({capabilities:{collection_paging:true,work_scoped_reads:true}} as any)).toThrow('Update Core');});
 it('accepts a Core advertising required scoped queries',async()=>{vi.resetModules();const fresh=await import('./api');expect(()=>fresh.requireCollectionPaging({capabilities:{collection_paging:true,work_scoped_reads:true,edition_membership_edit:true,resource_grouping_scoped:true,resource_members_paging:true,resource_preview_paging:true}} as any)).not.toThrow();});
});

it('rejects a Core missing scoped resource grouping',async()=>{vi.resetModules();const fresh=await import('./api');expect(()=>fresh.requireCollectionPaging({capabilities:{collection_paging:true,work_scoped_reads:true,edition_membership_edit:true}} as any)).toThrow('Update Core');});

it('rejects a Core missing member pages',async()=>{vi.resetModules();const fresh=await import('./api');expect(()=>fresh.requireCollectionPaging({capabilities:{collection_paging:true,work_scoped_reads:true,edition_membership_edit:true,resource_grouping_scoped:true}} as any)).toThrow('Update Core');});

it('rejects a Core missing preview pages',async()=>{vi.resetModules();const fresh=await import('./api');expect(()=>fresh.requireCollectionPaging({capabilities:{collection_paging:true,work_scoped_reads:true,edition_membership_edit:true,resource_grouping_scoped:true,resource_members_paging:true}} as any)).toThrow('Update Core');});
