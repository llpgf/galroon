import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import i18n from 'i18next';
import type {CollectionQuery} from './useCollectionPage';
import {LibraryBrowser,savedCollectionPageSize,savedCollectionSort} from './LibraryBrowser';

const seen=vi.hoisted(()=>({calls:[] as {request:CollectionQuery;limit?:number;infinite?:boolean}[]}));
vi.mock('./useCollectionPage',()=>({useCollectionPage:(request:CollectionQuery,_generation:number,limit?:number,infinite?:boolean)=>{seen.calls.push({request,limit,infinite});return {data:{items:[],total:120,filtered:120,next:'c2',options:{studio:[],year:[],language:[],tag:[]},status_counts:{all:120}},pageIndex:0,loading:false,loadingMore:false,loadMore:()=>{}};}}));
vi.mock('./api',()=>({api:vi.fn(),readApi:vi.fn()}));
const noop=()=>{};
let stored:Record<string,string>={};

beforeEach(async()=>{
 stored={};seen.calls=[];
 vi.stubGlobal('localStorage',{getItem:(key:string)=>stored[key]??null,setItem:(key:string,value:string)=>{stored[key]=value;}});
 vi.stubGlobal('navigator',{language:'zh-TW'});vi.stubGlobal('document',{documentElement:{lang:''}});
 await import('./i18n');await i18n.changeLanguage('zh-TW');
});
afterEach(()=>vi.unstubAllGlobals());

it('restores saved collection sort and page size, falling back safely on unknown values',()=>{
 expect(savedCollectionSort()).toBe('added');expect(savedCollectionPageSize()).toBe('60');
 stored={'galroon.collectionSort':'newest','galroon.collectionPageSize':'150'};
 expect(savedCollectionSort()).toBe('newest');expect(savedCollectionPageSize()).toBe('150');
 const html=renderToStaticMarkup(<LibraryBrowser generation={0} query="" setQuery={noop} status="all" setStatus={noop} safe canWrite onAdd={noop} onSource={noop} onSelect={noop} onStudio={noop} titleMode="display" setTitleMode={noop}/>);
 expect(seen.calls.at(-1)?.request.sort).toBe('newest');
 expect(seen.calls.at(-1)?.limit).toBe(150);
 expect(seen.calls.at(-1)?.infinite).toBe(false);
 expect(html).toContain('每頁 100 部');expect(html).toContain('每頁 150 部');expect(html).toContain('每頁 200 部');expect(html).toContain('無限捲動');
 stored={'galroon.collectionSort':'bogus','galroon.collectionPageSize':'999'};
 expect(savedCollectionSort()).toBe('added');expect(savedCollectionPageSize()).toBe('60');
});

it('switches to infinite scroll mode when saved as infinite',()=>{
 stored={'galroon.collectionSort':'oldest','galroon.collectionPageSize':'infinite'};
 const html=renderToStaticMarkup(<LibraryBrowser generation={0} query="" setQuery={noop} status="all" setStatus={noop} safe canWrite onAdd={noop} onSource={noop} onSelect={noop} onStudio={noop} titleMode="display" setTitleMode={noop}/>);
 expect(seen.calls.at(-1)?.request.sort).toBe('oldest');
 expect(seen.calls.at(-1)?.limit).toBe(100);
 expect(seen.calls.at(-1)?.infinite).toBe(true);
 expect(html).toContain('載入更多');
});
