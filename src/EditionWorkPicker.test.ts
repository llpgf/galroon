import {describe,it,expect} from 'vitest';
import {membershipCount,validateMembershipPage} from './EditionWorkPicker';
const base={id:'e',revision:4,collection_revision:8,library_id:'l',work_count:100000};
describe('bounded edition membership',()=>{
 it('retains the count of unseen members while tracking only explicit changes',()=>{expect(membershipCount(base,{a:{title:'A',selected:false},b:{title:'B',selected:true}})).toBe(100000);expect(membershipCount(null,{b:{title:'B',selected:true}})).toBe(1);});
 it('requires complete exact page coverage and the pinned revisions',()=>{const page={ids:['a','b'],members:['b'],revision:4,collection_revision:8,library_id:'l'};expect([...validateMembershipPage(page,['a','b'],base)]).toEqual(['b']);for(const changed of [{ids:['a']},{ids:['a','a']},{members:['outside']},{members:['b','b']},{revision:5},{collection_revision:9},{library_id:'other'}])expect(()=>validateMembershipPage({...page,...changed},['a','b'],base)).toThrow('could not be verified');});
});
