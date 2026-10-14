import {beforeEach,afterEach,expect,it,vi} from 'vitest';
const native=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke:native.invoke,isTauri:()=>true}));
vi.mock('./clientDiagnostics',()=>({reportClientError:vi.fn()}));
import {api,artworkBlob,readApi,setSession,connect,connectionSnapshot,RepairRequiredError,CollectionChangedError,SessionChangedError} from './api';
const remote={url:'ipc://core',token:'',remote:true,library_id:'library',device_id:'device',api_min:1,api_max:1};
function envelope(status:number,body:string|Uint8Array,headers:Record<string,string>={}){const metadata=new TextEncoder().encode(JSON.stringify({status,headers})),data=typeof body==='string'?new TextEncoder().encode(body):body;const result=new Uint8Array(4+metadata.length+data.length);new DataView(result.buffer).setUint32(0,metadata.length);result.set(metadata,4);result.set(data,4+metadata.length);return result.buffer;}
beforeEach(()=>{vi.useFakeTimers();native.invoke.mockReset();vi.stubGlobal('fetch',vi.fn());setSession(remote);});
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();vi.unstubAllGlobals();});
it('accepts a token-free remote session and sends JSON reads and writes through IPC',async()=>{
 native.invoke.mockResolvedValueOnce(remote);await connect();expect(connectionSnapshot().phase).toBe('connected');native.invoke.mockResolvedValue(envelope(200,'{"ok":true}'));
 await expect(api('/works')).resolves.toEqual({ok:true});await api('/works/w',{revision:2});
 expect(native.invoke).toHaveBeenCalledWith('core_request',{method:'GET',path:'/api/works',body:null,contentType:null,libraryId:'library',deviceId:'device'});
 expect(native.invoke).toHaveBeenCalledWith('core_request',{method:'POST',path:'/api/works/w',body:Array.from(new TextEncoder().encode('{"revision":2}')),contentType:'application/json',libraryId:'library',deviceId:'device'});expect(fetch).not.toHaveBeenCalled();
});
it('returns image bytes unchanged without base64 or JSON conversion',async()=>{
 const image=new Uint8Array([137,80,78,71,0,255,128]);native.invoke.mockResolvedValue(envelope(200,image,{'content-type':'image/png'}));const blob=await artworkBlob('https://image.invalid/x',new AbortController().signal);
 expect(blob.type).toBe('image/png');expect(new Uint8Array(await blob.arrayBuffer())).toEqual(image);expect(fetch).not.toHaveBeenCalled();
});
it.each([403,409,503])('preserves ordinary %i application errors',async status=>{
 native.invoke.mockResolvedValue(envelope(status,'{"error":"Original failure"}'));await expect(api('/works')).rejects.toMatchObject({message:'Original failure',status});expect(connectionSnapshot().phase).toBe('connected');expect(vi.getTimerCount()).toBe(0);
});
it('preserves collection_changed semantics and identity header checks',async()=>{
 native.invoke.mockResolvedValue(envelope(409,'{"code":"collection_changed"}'));await expect(api('/works')).rejects.toBeInstanceOf(CollectionChangedError);expect(connectionSnapshot().phase).toBe('collection_changed');
 setSession(remote);native.invoke.mockResolvedValue(envelope(200,'{}',{'x-galroon-device':'other'}));await expect(api('/works')).rejects.toBeInstanceOf(CollectionChangedError);
});
it('stops reconnecting after credential_invalid, including after a restart',async()=>{
 native.invoke.mockResolvedValue(envelope(401,'{"code":"credential_invalid"}'));await expect(api('/works')).rejects.toBeInstanceOf(RepairRequiredError);expect(connectionSnapshot().phase).toBe('repair_required');await vi.advanceTimersByTimeAsync(120000);expect(native.invoke).toHaveBeenCalledTimes(1);await expect(connect()).rejects.toBeInstanceOf(RepairRequiredError);expect(native.invoke).toHaveBeenCalledTimes(1);
 setSession(remote);native.invoke.mockRejectedValue('credential_invalid');await expect(connect()).rejects.toBeInstanceOf(RepairRequiredError);await vi.advanceTimersByTimeAsync(120000);expect(native.invoke).toHaveBeenCalledTimes(2);expect(connectionSnapshot().phase).toBe('repair_required');
});
it('recognizes revoked pairing from artwork errors too',async()=>{
 native.invoke.mockResolvedValue(envelope(401,'{"code":"credential_invalid"}'));await expect(artworkBlob('fixture',new AbortController().signal)).rejects.toBeInstanceOf(RepairRequiredError);expect(connectionSnapshot().phase).toBe('repair_required');expect(vi.getTimerCount()).toBe(0);
});
it('discards late IPC results after cancellation or a session switch',async()=>{
 let resolve!:(value:ArrayBuffer)=>void;native.invoke.mockReturnValue(new Promise<ArrayBuffer>(r=>{resolve=r;}));const abort=new AbortController();const read=readApi('/works',abort.signal);const checked=expect(read).rejects.toMatchObject({name:'AbortError'});abort.abort();await checked;resolve(envelope(200,'[]'));expect(connectionSnapshot().phase).toBe('connected');
 native.invoke.mockReturnValue(new Promise<ArrayBuffer>(r=>{resolve=r;}));const stale=api('/works');const rejected=expect(stale).rejects.toBeInstanceOf(SessionChangedError);setSession({...remote,device_id:'other'});resolve(envelope(200,'[]'));await rejected;
});
