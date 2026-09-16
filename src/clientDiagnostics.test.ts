import {describe,it,expect,vi,beforeEach} from 'vitest';
const native=vi.hoisted(()=>({invoke:vi.fn().mockResolvedValue(undefined),desktop:true}));
vi.mock('@tauri-apps/api/core',()=>({invoke:native.invoke,isTauri:()=>native.desktop}));
import {reportClientError} from './clientDiagnostics';
describe('desktop diagnostics',()=>{beforeEach(()=>{native.desktop=true;native.invoke.mockClear();});it('records bounded errors without relying on a Core session',()=>{reportClientError(new Error('Offline fixture'));expect(native.invoke).toHaveBeenCalledWith('record_client_error',{message:expect.stringContaining('Offline fixture')});reportClientError('x'.repeat(9000));expect(native.invoke.mock.calls[1][1].message.length).toBe(4000);});it('does not send browser error reports to the Core',()=>{native.desktop=false;reportClientError(new Error('Web fixture'));expect(native.invoke).not.toHaveBeenCalled();});});
