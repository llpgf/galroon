import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(),isTauri:()=>false}));
vi.mock('./clientDiagnostics',()=>({reportClientError:vi.fn()}));
import {webPair,webLogin,webAccessTransport,ApiRequestError} from './api';
const fetchMock=vi.fn();
beforeEach(()=>{fetchMock.mockReset();vi.stubGlobal('fetch',fetchMock);});
afterEach(()=>vi.unstubAllGlobals());
describe('web pairing API',()=>{
 it('posts a code with cookies enabled and no bearer or role',async()=>{fetchMock.mockResolvedValue(new Response(JSON.stringify({role:'web'})));await webPair('https://nas.local','ABCD-1234-ABCD');expect(fetchMock).toHaveBeenCalledWith('https://nas.local/api/access/pair-web',expect.objectContaining({method:'POST',credentials:'include',headers:{'Content-Type':'application/json'},body:JSON.stringify({code:'ABCD-1234-ABCD'})}));});
 it('preserves 403 status for password form switching',async()=>{fetchMock.mockResolvedValue(new Response(JSON.stringify({error:'LAN password login disabled'}),{status:403}));const error=await webLogin('https://nas.local','password','browser').catch(e=>e);expect(error).toBeInstanceOf(ApiRequestError);expect(error.status).toBe(403);});
 it('uses access status transport and can fall back to context transport',async()=>{fetchMock.mockResolvedValueOnce(new Response(JSON.stringify({transport:'lan'})));expect(await webAccessTransport('https://nas.local',new AbortController().signal)).toBe('lan');expect(fetchMock).toHaveBeenCalledTimes(1);fetchMock.mockResolvedValueOnce(new Response(JSON.stringify({configured:true}))).mockResolvedValueOnce(new Response(JSON.stringify({core:{transport:'lan'}})));expect(await webAccessTransport('https://nas.local',new AbortController().signal)).toBe('lan');expect(fetchMock).toHaveBeenLastCalledWith('https://nas.local/api/context',expect.objectContaining({credentials:'include'}));});
});
