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
 for(const response of [new Response('x',{headers:{'content-type':'image/jpeg','x-galroon-library':'wrong'}}),new Response('<html/>',{headers:{'content-type':'text/html'}}),new Response(new Uint8Array(8*1024*1024+1),{headers:{'content-type':'image/png'}})]){
  vi.stubGlobal('fetch',vi.fn().mockResolvedValue(response));await expect(artworkBlob(url,new AbortController().signal)).rejects.toThrow();
 }
});
it('does not send a pre-cancelled request and aborts a hung request at55s',async()=>{
 vi.useFakeTimers();const fetcher=vi.fn((_url,options)=>new Promise((_resolve,reject)=>{options.signal.addEventListener('abort',()=>reject(new DOMException('cancelled','AbortError')));}));vi.stubGlobal('fetch',fetcher);
 const cancelled=new AbortController();cancelled.abort();await expect(artworkBlob(url,cancelled.signal)).rejects.toMatchObject({name:'AbortError'});expect(fetcher).not.toHaveBeenCalled();
 const pending=artworkBlob(url,new AbortController().signal),check=expect(pending).rejects.toMatchObject({name:'AbortError'});await vi.advanceTimersByTimeAsync(55000);await check;expect(vi.getTimerCount()).toBe(0);
});
