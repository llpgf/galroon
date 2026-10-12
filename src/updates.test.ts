import {afterEach,describe,expect,it,vi} from 'vitest';
vi.mock('./api',()=>({api:vi.fn()}));
import {api} from './api';
import {readUpdates,checkUpdates,setUpdateSettings,shouldShowUpdate,skippedVersion,skipVersion,type UpdateState} from './updates';
const state:UpdateState={current:'0.0.2',install_kind:'standalone',enabled:true,checking:false,last_checked:123,last_error:null,update_available:true,latest:{version:'0.6.0',name:'New release',url:'https://github.com/llpgf/galroon/releases/tag/v0.6.0',notes:'<b>Plain text</b>',published_at:null}};
afterEach(()=>{vi.unstubAllGlobals();vi.clearAllMocks();});
describe('release updates',()=>{
 it('only shows an available, unskipped release',()=>{expect(shouldShowUpdate(null,null)).toBe(false);expect(shouldShowUpdate({...state,latest:null},null)).toBe(false);expect(shouldShowUpdate({...state,update_available:false},null)).toBe(false);expect(shouldShowUpdate(state,null)).toBe(true);expect(shouldShowUpdate(state,'0.6.0')).toBe(false);expect(shouldShowUpdate(state,'0.5.0')).toBe(true);expect(shouldShowUpdate({...state,enabled:false,last_error:'Offline'},null)).toBe(true);});
 it('remembers a skipped version on this client',()=>{const storage=new Map<string,string>();vi.stubGlobal('localStorage',{getItem:(key:string)=>storage.get(key)??null,setItem:(key:string,value:string)=>storage.set(key,value)});expect(skippedVersion()).toBeNull();skipVersion('0.6.0');expect(storage.get('galroon.updates.skipped')).toBe('0.6.0');expect(skippedVersion()).toBe('0.6.0');});
 it('tolerates unavailable client storage',()=>{vi.stubGlobal('localStorage',{getItem:()=>{throw new Error('Blocked');},setItem:()=>{throw new Error('Blocked');}});expect(skippedVersion()).toBeNull();expect(()=>skipVersion('0.6.0')).not.toThrow();});
 it('only calls Core routes, including manual checks with automatic checks off',async()=>{vi.mocked(api).mockResolvedValue(state);expect(await readUpdates()).toEqual(state);expect(api).toHaveBeenLastCalledWith('/updates');await setUpdateSettings(false);expect(api).toHaveBeenLastCalledWith('/updates/settings',{enabled:false});await checkUpdates();expect(api).toHaveBeenLastCalledWith('/updates/check',{});});
});
