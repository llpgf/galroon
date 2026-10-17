import {beforeEach,afterEach,it,expect,vi} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import i18n from 'i18next';
import type {Work} from './api';
import type {CollectionQuery} from './useCollectionPage';
import {HomePanel,savedSort,savedView,sortDate} from './HomePanel';
import {releaseReference,type Release} from './ReleaseCalendar';
const seen=vi.hoisted(()=>({requests:[] as CollectionQuery[]}));
vi.mock('./useCollectionPage',()=>({useCollectionPage:(request:CollectionQuery)=>{seen.requests.push(request);return {data:{items:[],total:0,filtered:0,options:{studio:[],year:[],language:[],tag:[]},status_counts:{}},pageIndex:0};}}));
vi.mock('./api',()=>({api:vi.fn(()=>new Promise(()=>{})),readApi:vi.fn()}));
const noop=()=>{};
let stored:Record<string,string>={};
beforeEach(async()=>{
 stored={};seen.requests=[];
 vi.stubGlobal('localStorage',{getItem:(key:string)=>stored[key]??null,setItem:(key:string,value:string)=>{stored[key]=value;}});vi.stubGlobal('navigator',{language:'zh-TW'});vi.stubGlobal('document',{documentElement:{lang:''}});
 await import('./i18n');await i18n.changeLanguage('zh-TW');
});
afterEach(()=>vi.unstubAllGlobals());
const work=(patch:Partial<Work>):Work=>({id:'w',title:'作品',original_title:'',vndb_id:null,description:'',cover:'',developer:'',released:'',tags:[],aliases:[],status:'playing',favorite:false,notes:'',revision:1,resources:0,...patch});

it('opens My play as a board sorted by date added unless the viewer chose otherwise',()=>{
 expect(savedView()).toBe('board');expect(savedSort()).toBe('status_changed');
 const html=renderToStaticMarkup(<HomePanel generation={0} safe titleMode="display" onSelect={noop} onCollection={noop} onReference={noop}/>);
 expect(html).toContain('class="home-board"');expect(html).toContain('<option value="status_changed" selected="">加入日期</option><option value="released">上市日期</option>');
 // Every status column asks Core for the chosen order; recent additions and favourites keep theirs.
 const statuses=seen.requests.filter(r=>!['all'].includes(r.status));
 expect(statuses.map(r=>r.status)).toEqual(['playing','completed','backlog','on_hold','dropped']);expect(statuses.every(r=>r.sort==='status_changed')).toBe(true);
 expect(seen.requests.filter(r=>r.status==='all').every(r=>r.sort==='added')).toBe(true);
 stored={'galroon.homeStatusView':'tabs','galroon.homeStatusSort':'released'};expect(savedView()).toBe('tabs');expect(savedSort()).toBe('released');
 seen.requests=[];const tabs=renderToStaticMarkup(<HomePanel generation={0} safe titleMode="display" onSelect={noop} onCollection={noop} onReference={noop}/>);
 expect(tabs).toContain('role="tab"');expect(seen.requests.find(r=>r.status==='playing')?.sort).toBe('released');
 stored={'galroon.homeStatusSort':'bogus'};expect(savedSort()).toBe('status_changed');
});

it('shows the date that matches the active sort',()=>{
 const day=Date.UTC(2026,9,17,12)/1000;
 expect(sortDate(work({status_since:day}),'status_changed','en-CA')).toBe('2026-10-17');
 expect(sortDate(work({status_since:null}),'status_changed','en-CA')).toBe('');
 expect(sortDate(work({released:'2004-04-28',status_since:day}),'released','en-CA')).toBe('2004-04-28');
});

it('turns a release outside the collection into a VNDB reference page',()=>{
 const release:Release={release_id:'r1',vn_id:'v17',released:'2026-11-27',title:'Romaji',original_title:'原題',producers:['Key'],image:{url:'https://t.vndb.org/cv/1.jpg'}};
 expect(releaseReference(release)).toEqual({id:'v17',title:'Romaji',alttitle:'原題',released:'2026-11-27',image:{url:'https://t.vndb.org/cv/1.jpg'}});
 expect(releaseReference({...release,title:'',original_title:'原題',image:null})).toStrictEqual({id:'v17',title:'原題',alttitle:undefined,released:'2026-11-27',image:undefined});
});
