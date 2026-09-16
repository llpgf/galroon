import {APP_VERSION} from './version';
import {beforeEach,describe,it,expect,vi} from 'vitest';
import {browserDiagnostics,clearBrowserDiagnostics,recordBrowserError} from './browserDiagnostics';
const data=new Map<string,string>();
beforeEach(()=>{data.clear();vi.stubGlobal('sessionStorage',{getItem:(k:string)=>data.get(k)??null,setItem:(k:string,v:string)=>data.set(k,v),removeItem:(k:string)=>data.delete(k)});clearBrowserDiagnostics();});
describe('browser diagnostics',()=>{
 it('preserves historical versions and records the current frontend version',()=>{
  data.set('galroon.diagnostics.v1',JSON.stringify([
   {time:'2026-01-01',version:'0.0.1',message:'Historical failure'},
   {time:'2026-01-02',message:'Unversioned failure'},
   {time:'2026-01-03',version:'token=unsafe',message:'Invalid version'}
  ]));
  recordBrowserError('Current failure');
  const entries=browserDiagnostics().text.split('\n').map(line=>JSON.parse(line));
  expect(entries.map(v=>v.version)).toEqual(['0.0.1','unknown','unknown',APP_VERSION]);
  expect(entries.map(v=>v.message)).toEqual(['Historical failure','Unversioned failure','Invalid version','Current failure']);
 });
 it('keeps newest memory entries when existing storage remains readable but writes fail',()=>{
  recordBrowserError('Persisted before quota');
  const getItem=(k:string)=>data.get(k)??null;
  vi.stubGlobal('sessionStorage',{getItem,setItem:()=>{throw Error('quota');},removeItem:()=>{throw Error('blocked');}});
  recordBrowserError('Newest unsaved failure');recordBrowserError('Second unsaved failure');
  expect(browserDiagnostics().text).toContain('Newest unsaved failure');expect(browserDiagnostics().text).toContain('Second unsaved failure');
  expect([...data.values()][0]).not.toContain('Newest unsaved failure');
  clearBrowserDiagnostics();expect(browserDiagnostics().text).toBe('');expect(browserDiagnostics().error).not.toBeNull();
  vi.stubGlobal('sessionStorage',{getItem,setItem:(k:string,v:string)=>data.set(k,v),removeItem:(k:string)=>data.delete(k)});
  recordBrowserError('Storage recovered');expect(browserDiagnostics().text).not.toContain('Persisted before quota');expect(browserDiagnostics().text).toContain('Storage recovered');expect(browserDiagnostics().error).toBeNull();
 });
 it('scrubs labeled secrets and URLs before storage',()=>{recordBrowserError('Failure {"access_token":"sensitive", "password":"escaped\\" value"} Bearer credential https://site.invalid/private?secret=yes');const raw=[...data.values()][0];for(const secret of ['sensitive','escaped','credential','site.invalid'])expect(raw).not.toContain(secret);expect(browserDiagnostics().text).toContain('Failure');});
 it('bounds entry count and serialized storage while keeping newest events',()=>{for(let i=0;i<250;i++)recordBrowserError(`${i}:`+'界'.repeat(4000));const raw=[...data.values()][0];expect(raw.length).toBeLessThanOrEqual(65536);expect(JSON.parse(raw).length).toBeLessThanOrEqual(100);expect(browserDiagnostics().text).toContain('249:');expect(JSON.parse(raw).some((v:{message:string})=>v.message.startsWith('0:'))).toBe(false);});
 it('survives denied storage without causing another exception and can clear memory',()=>{vi.stubGlobal('sessionStorage',{getItem:()=>{throw Error('blocked');},setItem:()=>{throw Error('quota');},removeItem:()=>{throw Error('blocked');}});expect(()=>recordBrowserError('Offline failure')).not.toThrow();expect(browserDiagnostics().text).toContain('Offline failure');expect(browserDiagnostics().error).not.toBeNull();clearBrowserDiagnostics();expect(browserDiagnostics().text).toBe('');});
 it('ignores malformed storage and clears stored reports',()=>{data.set('galroon.diagnostics.v1','{');expect(()=>recordBrowserError('Recovered')).not.toThrow();expect(browserDiagnostics().text).toContain('Recovered');clearBrowserDiagnostics();expect(data.size).toBe(0);expect(browserDiagnostics().text).toBe('');});
});


