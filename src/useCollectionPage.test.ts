import {describe,it,expect} from 'vitest';
import {isBusy,retryWhileBusy} from './useCollectionPage';
const busy=()=>new Error('Other collection queries are running. Retry in a moment.');
describe('collection reads while Core is busy',()=>{
 it('retries busy refusals until the read succeeds',async()=>{let calls=0;const read=async()=>{if(++calls<3)throw busy();return 'page';};expect(await retryWhileBusy(read,new AbortController().signal,[1,1,1])).toBe('page');expect(calls).toBe(3);});
 it('gives up after the last delay and surfaces the busy error',async()=>{let calls=0;await expect(retryWhileBusy(async()=>{calls++;throw busy();},new AbortController().signal,[1,1])).rejects.toThrow('Retry in a moment');expect(calls).toBe(3);});
 it('does not retry other failures',async()=>{let calls=0;await expect(retryWhileBusy(async()=>{calls++;throw new Error('Invalid collection query');},new AbortController().signal,[1])).rejects.toThrow('Invalid');expect(calls).toBe(1);});
 it('stops waiting when the query is cancelled',async()=>{const controller=new AbortController();let calls=0;const pending=retryWhileBusy(async()=>{calls++;throw busy();},controller.signal,[10000]);controller.abort();await expect(pending).rejects.toThrow('cancelled');expect(calls).toBe(1);});
 it('recognises only Core busy messages',()=>{expect(isBusy(busy())).toBe(true);expect(isBusy(new Error('Other work queries are running. Retry in a moment.'))).toBe(true);expect(isBusy(new Error('Request failed (400)'))).toBe(false);expect(isBusy('busy')).toBe(false);});
});
