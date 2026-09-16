import {describe,it,expect} from 'vitest';
import {discoverableCharacter,discoveryPath,manualDiscoveryPath,mergeDiscoveryResults,mergeResults,searchableDate} from './discovery';
describe('metadata discovery navigation',()=>{
 it('preserves Unicode titles and separates trait identity from its label',()=>{
  const url=new URL(discoveryPath({kind:'title',value:'マガルミナ & #1',label:'Title'},'works',2,false),'https://local.test');
  expect(url.searchParams.get('value')).toBe('マガルミナ & #1');expect(url.searchParams.get('page')).toBe('2');
  const trait=new URL(discoveryPath({kind:'trait',value:'i4',label:'Hair · Black'},'characters',1,true),'https://local.test');
  expect(trait.searchParams.get('value')).toBe('i4');expect(trait.searchParams.get('spoilers')).toBe('true');expect(trait.searchParams.has('label')).toBe(false);
 });
 it('deduplicates page overlaps and retains existing order',()=>{expect(mergeResults([{id:'c1',name:'One'}],[{id:'c1',name:'Updated'},{id:'c2',name:'Two'}])).toEqual([{id:'c1',name:'Updated'},{id:'c2',name:'Two'}]);});
 it('hides characters whose appearances are all spoilers',()=>{const character={id:'c1',name:'Hidden',vns:[{id:'v1',spoiler:2}]};expect(discoverableCharacter(character,false)).toBe(false);expect(discoverableCharacter(character,true)).toBe(true);expect(discoverableCharacter({...character,vns:[...character.vns,{id:'v2',spoiler:0}]},false)).toBe(true);});
 it('does not make unknown dates into search links',()=>{for(const date of ['2026','2026-06','2026-06-26'])expect(searchableDate(date)).toBe(true);for(const date of ['',null,undefined,'TBA','unknown'])expect(searchableDate(date)).toBe(false);});
});

describe('manual discovery union',()=>{
 it('keeps provider ranking and metadata while appending manual-only results',()=>{const provider=[{id:'v2',title:'Rank one',developers:[{id:'p1',name:'Source'}]},{id:'v1',title:'Rank two'}];const saved=JSON.stringify(provider);expect(mergeDiscoveryResults(provider,[{id:'v2',title:'Subset'},{id:'v3',title:'Manual'}])).toEqual([...provider,{id:'v3',title:'Manual'}]);expect(JSON.stringify(provider)).toBe(saved);});
 it('keeps provider page and manual cursor independent at a common revision',()=>{const query={kind:'studio' as const,value:'会社 & Co',label:'Company'};const provider=new URL(discoveryPath(query,'works',3,false,17),'https://fixture');const manual=new URL(manualDiscoveryPath(query,'works',false,'v50',17),'https://fixture');expect(provider.searchParams.get('revision')).toBe('17');expect(manual.searchParams.get('revision')).toBe('17');expect(manual.searchParams.get('after')).toBe('v50');expect(manual.searchParams.has('page')).toBe(false);expect(provider.searchParams.has('after')).toBe(false);expect(manual.searchParams.get('value')).toBe('会社 & Co');});
});
