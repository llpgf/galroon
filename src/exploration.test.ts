import {describe,it,expect} from 'vitest';
import {appendWorks,safeExternalUrl,visibleTraits,initials,imageAllowed,visibleCharacter,voiceFor,plainDescription,type Exploration} from './exploration';
describe('exploration presentation boundaries',()=>{
 it('builds Unicode name avatars without inventing photos',()=>{expect(initials(' Asami Imai ')).toBe('AI');expect(initials('huke')).toBe('HU');expect(initials('今井麻美')).toBe('今井');expect(initials('')).toBe('?');});
 it('keeps work-specific spoiler relations separate',()=>{const c={id:'c1',name:'Name',vns:[{id:'v1',spoiler:2},{id:'v2',spoiler:0}]};expect(visibleCharacter(c,'v1',false)).toBe(false);expect(visibleCharacter(c,'v2',false)).toBe(true);expect(visibleCharacter(c,'v1',true)).toBe(true);expect(visibleCharacter({id:'c1',name:'Name'},'v1',false)).toBe(false);});
 it('pairs by character ID and preserves multiple actors and credited aliases',()=>{const v=(character:string,id:string,aid:number)=>({character:{id:character,name:''},staff:{id,aid,name:'Name'}});const data={vn:{id:'v1',staff:[],relations:[],va:[v('c1','s1',1),v('c1','s1',1),v('c1','s1',2),v('c1','s2',3),v('c2','s3',4)]},characters:[],more:false} satisfies Exploration;expect(voiceFor(data,'c1')).toHaveLength(3);});
 it('hides rated imagery and explicit spoiler text',()=>{expect(imageAllowed({url:'https://example.com',sexual:1},false)).toBe(false);expect(imageAllowed({url:'https://example.com'},true)).toBe(false);expect(plainDescription('[b]Intro[/b] [spoiler]Secret[/spoiler]')).toBe('Intro [Spoiler hidden]');expect(plainDescription('[spoiler]Secret[/spoiler]',true)).toBe('Secret');});
});

describe('profile metadata',()=>{
 it('rejects executable external links',()=>{expect(safeExternalUrl('javascript:alert(1)')).toBeNull();expect(safeExternalUrl('data:text/html,x')).toBeNull();expect(safeExternalUrl('https://vndb.org/s1')).toBe('https://vndb.org/s1');});
 it('keeps false traits hidden and honours spoiler and content settings',()=>{const trait={id:'i1',name:'Trait',group_name:'Test',spoiler:0,lie:false};const character={id:'c1',name:'Name',traits:[trait,{...trait,id:'i2',lie:true},{...trait,id:'i3',spoiler:2},{...trait,id:'i4',sexual:true}]};expect(visibleTraits(character,false,true).map(t=>t.id)).toEqual(['i1']);expect(visibleTraits(character,true,false).map(t=>t.id)).toEqual(['i1','i3','i4']);});
 it('merges overlapping credit pages without duplicate works',()=>{expect(appendWorks([{id:'v1',title:'Old'},{id:'v2',title:'Two'}],[{id:'v1',title:'Updated'},{id:'v3',title:'Three'}])).toEqual([{id:'v1',title:'Updated'},{id:'v2',title:'Two'},{id:'v3',title:'Three'}]);});
});
