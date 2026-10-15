import {afterEach,expect,it,vi} from 'vitest';
import {clearClientArtwork,clientArtworkGeneration,clientArtworkStats,keepArtwork,keptArtwork} from './clientArtworkCache';
import {setSession,signOut} from './api';
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>false,invoke:vi.fn()}));
vi.mock('./clientDiagnostics',()=>({reportClientError:vi.fn()}));
afterEach(()=>{clearClientArtwork();vi.restoreAllMocks();vi.unstubAllGlobals();});
it('bounds image bodies and clears local URLs without contacting Core',()=>{
 const revoke=vi.spyOn(URL,'revokeObjectURL').mockImplementation(()=>{}),network=vi.fn();vi.stubGlobal('fetch',network);clearClientArtwork();
 keepArtwork('a','blob:a',40*1024*1024);keepArtwork('b','blob:b',30*1024*1024);
 expect(keptArtwork('a')).toBeUndefined();expect(keptArtwork('b')).toBe('blob:b');expect(clientArtworkStats().bytes).toBe(30*1024*1024);expect(revoke).toHaveBeenCalledWith('blob:a');
 const generation=clientArtworkGeneration();clearClientArtwork();expect(clientArtworkGeneration()).toBeGreaterThan(generation);expect(clientArtworkStats().images).toBe(0);expect(revoke).toHaveBeenCalledWith('blob:b');expect(network).not.toHaveBeenCalled();
});
it('does not reuse previous Core artwork after a session change or logout',async()=>{
 vi.spyOn(URL,'revokeObjectURL').mockImplementation(()=>{});setSession({url:'http://core-a',token:'fixture'});keepArtwork('same-url','blob:a',10);
 setSession({url:'http://core-b',token:'fixture'});expect(keptArtwork('same-url')).toBeUndefined();keepArtwork('same-url','blob:b',10);
 vi.stubGlobal('fetch',vi.fn().mockResolvedValue(new Response('{}',{headers:{'content-type':'application/json'}})));await signOut();expect(keptArtwork('same-url')).toBeUndefined();
});
