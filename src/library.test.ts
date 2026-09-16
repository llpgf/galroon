import {describe,it,expect} from 'vitest';
import {catalogEntries,emptyFilters,facetOptions,matches,titleFor,tagNames} from './library';
import type {Work,Root,Resource,Edition} from './api';
const work=(id:string,extra:Partial<Work>={}):Work=>({id,title:id,original_title:id,vndb_id:null,description:'',cover:'',developer:'',released:'',tags:[],aliases:[],status:'backlog',favorite:false,notes:'',revision:1,resources:0,...extra});
const roots:Root[]=[{id:'online',path:'C:/games',label:'Online',role:'source',online:true,missing:0,unverified:0},{id:'offline',path:'D:/games',label:'Offline',role:'source',online:false,missing:0,unverified:0}];
const resource=(id:string,works:string[],extra:Partial<Resource>={}):Resource=>({id,root_id:'online',root_path:'C:/games',missing:0,unverified:0,revision:1,title:id,path:id,kind:'archive',work_id:works[0],release:'',files:1,bytes:1,bindings:works.map(work_id=>({work_id,release_id:null,role:'main'})),...extra});
const edition=(works:string[],languages:string[]):Edition=>({id:'edition',label:'Edition',languages,edition_type:'original',released_date:'',vndb_id:null,work_ids:works,revision:1});
describe('collection exploration',()=>{
 it('combines metadata filters with alias search and reading state',()=>{
  const [e]=catalogEntries([work('w',{title:'English name',original_title:'原題',aliases:['別名'],developer:'Key',released:'2004-04-28',tags:['Drama'],favorite:true})],[resource('r',['w'])],roots,[edition(['w'],['ja','en'])]);
  expect(matches(e,{studio:'value:key',year:'value:2004',language:'value:ja',tag:'value:drama',availability:'available'},'別名 KEY','favorite')).toBe(true);
  expect(matches(e,{...emptyFilters,language:'value:fr'},'別名','all')).toBe(false);
  expect(matches(e,emptyFilters,'原題','completed')).toBe(false);
 });
 it('keeps available copies and missing copies independently searchable',()=>{
  const [e]=catalogEntries([work('w')],[resource('good',['w']),resource('missing',['w'],{missing:1}),resource('offline',['w'],{root_id:'offline'})],roots,[]);
  for(const state of ['available','missing','offline'])expect(matches(e,{...emptyFilters,availability:state},'','all')).toBe(true);
  expect(matches(e,{...emptyFilters,availability:'unverified'},'','all')).toBe(false);
 });
 it('does not advertise unverified, disconnected or empty resources as available',()=>{
  const entries=catalogEntries(['unverified','offline','empty','unknown','legacy','none'].map(id=>work(id)),[resource('u',['unverified'],{unverified:1}),resource('o',['offline'],{root_id:'offline'}),resource('e',['empty'],{files:0}),resource('x',['unknown'],{root_id:'missing-root'}),resource('legacy',['legacy'],{unknown:1})],roots,[]);
  for(const e of entries)expect(matches(e,{...emptyFilters,availability:'available'},'','all')).toBe(false);
  expect(entries.at(-1)?.states.has('no_resources')).toBe(true);
  expect(entries[2].states.has('unknown')).toBe(true);
 });
 it('uses shared edition languages and resource bindings without creating extra cards',()=>{
  const entries=catalogEntries([work('a'),work('b'),work('c')],[resource('shared',['a','b'])],roots,[edition(['a','b'],[' JA ','ja','EN'])]);
  expect(entries).toHaveLength(3);expect(entries.filter(e=>matches(e,{...emptyFilters,language:'value:ja'},'','all')).map(e=>e.work.id)).toEqual(['a','b']);
  expect(facetOptions(entries,'language')).toEqual([{value:'value:en',label:'en'},{value:'value:ja',label:'ja'}]);
  expect(matches(entries[2],{...emptyFilters,language:'unknown'},'','all')).toBe(true);
 });
 it('preserves unknown metadata and normalizes full-width search without inventing values',()=>{
  const [e]=catalogEntries([work('w',{title:'Ｆｏｏ',released:'unknown',tags:[{name:'Drama'},'Drama',42,{name:0}]})],[],[],[]);
  expect(matches(e,{...emptyFilters,year:'unknown',studio:'unknown'},'foo','all')).toBe(true);
  expect(facetOptions([e],'year')).toEqual([]);expect(tagNames(e.work.tags)).toEqual(['Drama']);
 });
 it('selects the original title while preserving the manual display title and fallback',()=>{
  const w=work('w',{title:'自訂中文名稱',original_title:'原題'});expect(titleFor(w,'display')).toBe('自訂中文名稱');expect(titleFor(w,'original')).toBe('原題');expect(w.title).toBe('自訂中文名稱');expect(titleFor({...w,original_title:''},'original')).toBe('自訂中文名稱');
 });
});

it('unions selected tags while intersecting status, favourites and other facets',()=>{
 const entries=catalogEntries([work('drama',{tags:['Drama'],favorite:true,developer:'Key'}),work('comedy',{tags:['Comedy'],favorite:true,developer:'Key'}),work('other',{tags:['Action'],favorite:true,developer:'Key'}),work('not-favourite',{tags:['Drama'],developer:'Key'}),work('finished',{tags:['Comedy'],favorite:true,status:'completed',developer:'Key'})],[],[],[]);
 const filters={...emptyFilters,tags:['value:drama','value:comedy'],favorites:true,studio:'value:key'};
 expect(entries.filter(e=>matches(e,filters,'','backlog')).map(e=>e.work.id)).toEqual(['drama','comedy']);
 expect(entries.filter(e=>matches(e,{...filters,tags:['value:comedy']},'','backlog')).map(e=>e.work.id)).toEqual(['comedy']);
 expect(entries.filter(e=>matches(e,{...emptyFilters,tags:[]},'','all'))).toHaveLength(5);
 expect(entries.filter(e=>matches(e,filters,'comedy','backlog')).map(e=>e.work.id)).toEqual(['comedy']);
});

it('inverts tag selections with exclusions taking priority, including exclude-only focus',()=>{
 const entries=catalogEntries([work('a',{tags:['Drama']}),work('b',{tags:['Comedy']}),work('both',{tags:['Drama','Comedy']}),work('neither',{tags:[]})],[],[],[]);
 const ids=(excludedTags:string[])=>entries.filter(e=>matches(e,{...emptyFilters,tags:['value:drama','value:comedy'],excludedTags},'','all')).map(e=>e.work.id);
 expect(ids([])).toEqual(['a','b','both']);
 expect(ids(['value:drama'])).toEqual(['b']);
 expect(ids(['value:comedy'])).toEqual(['a']);
 expect(ids(['value:drama','value:comedy'])).toEqual(['neither']);
 expect(entries.filter(e=>matches(e,{...emptyFilters,tags:[],excludedTags:['value:drama']},'','all'))).toHaveLength(4);
});

it('keeps custom-tag identity separate from a VNDB tag with the same name',()=>{
 const [a,b]=catalogEntries([work('a',{tags:['Drama']}),work('b',{tags:['Comedy']})],[],[],[]);
 a.customTags=[];b.customTags=['custom:drama'];
 expect(matches(a,{...emptyFilters,tags:['custom:drama']},'','all')).toBe(false);
 expect(matches(b,{...emptyFilters,tags:['custom:drama']},'','all')).toBe(true);
 expect(matches(b,{...emptyFilters,tags:['custom:drama'],excludedTags:['custom:drama']},'','all')).toBe(false);
 expect(matches(a,{...emptyFilters,tags:['custom:drama'],excludedTags:['custom:drama']},'','all')).toBe(true);
});

