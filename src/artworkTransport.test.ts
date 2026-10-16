import {afterEach,beforeEach,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>false,invoke:vi.fn()}));
vi.mock('./clientDiagnostics',()=>({reportClientError:vi.fn()}));
import {artworkBlob,setSession,SessionChangedError,CollectionChangedError} from './api';
const url='https://t.vndb.org/cv/01/1.jpg',session={url:'http://127.0.0.1:41001',token:'fixture',library_id:'a',device_id:'d'};
beforeEach(()=>{setSession(session);});afterEach(()=>{vi.unstubAllGlobals();vi.useRealTimers();});
it('uses only Core credentials and returns bounded raster bytes',async()=>{
 const fetcher=vi.fn().mockResolvedValue(new Response(new Uint8Array([1,2,3]),{headers:{'content-type':'image/jpeg'}}));vi.stubGlobal('fetch',fetcher);
 const blob=await artworkBlob(url,new AbortController().signal);expect(blob.type).toBe('image/jpeg');expect(blob.size).toBe(3);
 expect(fetcher.mock.calls[0][0]).toBe(session.url+'/api/artwork?url='+encodeURIComponent(url));expect(fetcher.mock.calls[0][1].headers.Authorization).toBe('Bearer fixture');expect(fetcher.mock.calls[0][1].credentials).toBe('omit');
});
it('rejects a late response after changing collection and cancels its body',async()=>{
 let finish!:(r:Response)=>void;const cancelled=vi.fn();vi.stubGlobal('fetch',()=>new Promise<Response>(resolve=>{finish=resolve;}));
 const pending=artworkBlob(url,new AbortController().signal),check=expect(pending).rejects.toBeInstanceOf(SessionChangedError);setSession({...session,library_id:'b'});
 finish(new Response(new ReadableStream({cancel:cancelled}),{headers:{'content-type':'image/jpeg'}}));await check;expect(cancelled).toHaveBeenCalledOnce();
});
it('rejects wrong Core identity, non-image data and excessive streamed data',async()=>{
 for(const response of [new Response('x',{headers:{'content-type':'image/jpeg','x-galroon-library':'wrong'}}),new Response('<html/>',{headers:{'content-type':'text/html'}}),new Response(new Uint8Array(16*1024*1024+1),{headers:{'content-type':'image/png'}})]){
  vi.stubGlobal('fetch',vi.fn().mockResolvedValue(response));await expect(artworkBlob(url,new AbortController().signal)).rejects.toThrow();
 }
});
it('does not send a pre-cancelled request and aborts a hung request at55s',async()=>{
 vi.useFakeTimers();const fetcher=vi.fn((_url,options)=>new Promise((_resolve,reject)=>{options.signal.addEventListener('abort',()=>reject(new DOMException('cancelled','AbortError')));}));vi.stubGlobal('fetch',fetcher);
 const cancelled=new AbortController();cancelled.abort();await expect(artworkBlob(url,cancelled.signal)).rejects.toMatchObject({name:'AbortError'});expect(fetcher).not.toHaveBeenCalled();
 const pending=artworkBlob(url,new AbortController().signal),check=expect(pending).rejects.toMatchObject({name:'AbortError'});await vi.advanceTimersByTimeAsync(55000);await check;expect(vi.getTimerCount()).toBe(0);
});

it('uses native Blob consumption for known lengths and enforces declared and received limits',async()=>{
 const response=new Response(new Uint8Array([1,2,3]),{headers:{'content-type':'image/png','content-length':'3'}}),blob=vi.spyOn(response,'blob');
 vi.stubGlobal('fetch',vi.fn().mockResolvedValue(response));expect((await artworkBlob(url,new AbortController().signal)).size).toBe(3);expect(blob).toHaveBeenCalledOnce();
 for(const length of ['1',String(16*1024*1024+1)]){vi.stubGlobal('fetch',vi.fn().mockResolvedValue(new Response(new Uint8Array(16*1024*1024+1),{headers:{'content-type':'image/png','content-length':length}})));await expect(artworkBlob(url,new AbortController().signal)).rejects.toThrow('Artwork exceeds size limit');}
});

it('stops invalid credentials from triggering more artwork requests until a new session is supplied',async()=>{
 const {RepairRequiredError}=await import('./api');
 const {clientArtworkGeneration}=await import('./clientArtworkCache');
 const fetcher=vi.fn().mockImplementation(()=>Promise.resolve(new Response('{"code":"credential_invalid"}',{status:401,headers:{'content-type':'application/json'}})));
 vi.stubGlobal('fetch',fetcher);
 await expect(artworkBlob(url,new AbortController().signal)).rejects.toBeInstanceOf(RepairRequiredError);
 const generation=clientArtworkGeneration();
 for(let n=0;n<3;n++)await expect(artworkBlob(url,new AbortController().signal)).rejects.toBeInstanceOf(RepairRequiredError);
 expect(fetcher).toHaveBeenCalledTimes(1);expect(clientArtworkGeneration()).toBe(generation);
 setSession({...session,token:'replacement'});
 fetcher.mockResolvedValue(new Response(new Uint8Array([1]),{headers:{'content-type':'image/jpeg'}}));
 expect((await artworkBlob(url,new AbortController().signal)).size).toBe(1);
 expect(fetcher).toHaveBeenCalledTimes(2);
});

it('invalidates cached images once when concurrent requests report revoked credentials',async()=>{
 const {api,RepairRequiredError}=await import('./api');
 const {clientArtworkGeneration}=await import('./clientArtworkCache');
 let release!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve;});
 const fetcher=vi.fn().mockImplementation(()=>{
  const response=new Response('{"code":"credential_invalid"}',{status:401,headers:{'content-type':'application/json'}});
  vi.spyOn(response,'json').mockImplementation(async()=>{await gate;return {code:'credential_invalid'};});return Promise.resolve(response);
 });
 vi.stubGlobal('fetch',fetcher);
 const generation=clientArtworkGeneration();
 const pending=Promise.allSettled([artworkBlob(url,new AbortController().signal),artworkBlob(url+'other',new AbortController().signal)]);
 await Promise.resolve();release();await pending;
 expect(clientArtworkGeneration()).toBe(generation+1);
 await expect(api('/context')).rejects.toBeInstanceOf(RepairRequiredError);
 expect(fetcher).toHaveBeenCalledTimes(2);
});

it('preserves a cover larger than 8 MiB over HTTP and the desktop proxy',async()=>{
 const bytes=new Uint8Array(10_369_393);bytes.set([255,216,255]);
 for(const remote of [false,true])for(const declared of [false,true]){
  setSession(remote?{...session,remote:true,proxy_url:'http://127.0.0.1:41002',proxy_key:'fixture-proxy'}:session);
  const headers:Record<string,string>={'content-type':'image/jpeg'};if(declared)headers['content-length']=String(bytes.length);
  vi.stubGlobal('fetch',vi.fn().mockResolvedValue(new Response(bytes,{headers})));
  const image=await artworkBlob(url,new AbortController().signal);
  expect(image.size).toBe(bytes.length);expect(image.type).toBe('image/jpeg');
  const saved=new Uint8Array(await image.arrayBuffer());expect(saved.every((value,i)=>value===bytes[i])).toBe(true);
 }
});
