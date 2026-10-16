import {beforeEach,afterEach,it,expect,vi} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import i18n from 'i18next';
import type {Work} from './api';
import {HomePanel} from './HomePanel';
import {LibraryBrowser} from './LibraryBrowser';
import {StatsPanel} from './StatsPanel';
import {CollectionWorkDetail} from './CollectionWorkDetail';
import {newCondition} from './smartLists';
const fixture=vi.hoisted(()=>({items:[] as Work[]}));
vi.mock('./useCollectionPage',()=>({useCollectionPage:()=>({data:{items:fixture.items,total:2,filtered:2,options:{studio:[],year:[],language:[],tag:[]},status_counts:{none:1,backlog:1}},pageIndex:0})}));
vi.mock('./api',()=>({api:vi.fn(),readApi:vi.fn()}));
vi.mock('./WorkExperience',()=>({WorkExperience:({actions}:{actions:React.ReactNode})=><div>{actions}</div>}));
const noop=()=>{};
beforeEach(async()=>{
 vi.stubGlobal('localStorage',{getItem:()=>null});vi.stubGlobal('navigator',{language:'zh-TW'});vi.stubGlobal('document',{documentElement:{lang:''}});await import('./i18n');await i18n.changeLanguage('zh-TW');
 fixture.items=['none','backlog'].map((status,i)=>({id:String(i),title:'作品 '+i,original_title:'Original '+i,vndb_id:null,description:'',cover:'',developer:'',released:'',tags:[],aliases:[],status,favorite:false,notes:'',revision:1,resources:0}));
});
afterEach(()=>vi.unstubAllGlobals());
it('keeps five home columns and adds the no-status count link',()=>{
 const html=renderToStaticMarkup(<HomePanel generation={0} safe titleMode="display" onSelect={noop} onCollection={noop}/>);
 expect(html).toContain('無狀態 1 部');expect(html.match(/role="tab"/g)).toHaveLength(5);expect(html).not.toContain('home-tab-none');expect(html).toContain('想玩');
});
it('offers no status as a collection segment and badges only the explicit status',()=>{
 const html=renderToStaticMarkup(<LibraryBrowser generation={0} query="" setQuery={noop} status="none" setStatus={noop} safe canWrite onAdd={noop} onSource={noop} onSelect={noop} onStudio={noop} titleMode="display" setTitleMode={noop}/>);
 expect(html).toContain('status-dot-none');expect(html).toContain('cover-status status-backlog');expect(html).not.toContain('cover-status status-none');expect(html).toContain('想玩');
});
it('includes no status in statistics while new smart conditions remain want-to-play',()=>{
 const html=renderToStaticMarkup(<StatsPanel generation={0} titleMode="display" onCollection={noop}/>);
 expect(html).toContain('seg-none');expect(html).toContain('無狀態');expect(html).toContain('想玩');expect(newCondition()).toMatchObject({predicate:{values:['backlog']}});
});
it('starts the work status selector with no status followed by want-to-play',()=>{
 const html=renderToStaticMarkup(<CollectionWorkDetail work={fixture.items[0]} canWrite safe titleMode="display" onBack={noop} onWork={noop} roots={[]} busy={false} act={async fn=>{await fn();}} refresh={async()=>{}} onEdit={noop} onGroup={noop} onReferences={noop} onAcquire={noop} onOrganize={noop}/>);
 expect(html).toContain('<option value="none" selected="">無狀態</option><option value="backlog">想玩</option>');expect(html.match(/<option /g)).toHaveLength(6);
});
