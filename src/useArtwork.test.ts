import {describe,expect,it,vi} from 'vitest';
import {keepArtwork,keptArtwork} from './useArtwork';
describe('kept artwork',()=>{
 it('reuses a decoded cover and releases the least recently used one past the limit',()=>{
  const revoke=vi.spyOn(URL,'revokeObjectURL').mockImplementation(()=>{});
  for(let i=0;i<600;i++)keepArtwork(`s${i}`,`blob:${i}`);
  expect(keptArtwork('s0')).toBe('blob:0');
  keepArtwork('s600','blob:600');
  expect(revoke).toHaveBeenCalledWith('blob:1');expect(keptArtwork('s1')).toBeUndefined();expect(keptArtwork('s0')).toBe('blob:0');
  revoke.mockRestore();
 });
});
