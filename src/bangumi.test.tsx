import {beforeEach,afterEach,describe,it,expect,vi} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import i18n from 'i18next';
import {BangumiRows} from './BangumiPanel';
import {bangumiStatus,bangumiLocalStatus,bangumiChoice,defaultBangumiAction,type BangumiRow} from './bangumi';
vi.mock('./api',()=>({api:vi.fn(),readApi:vi.fn()}));
const row=(kind:string):BangumiRow=>({subject_id:1,title:'A <game>',work_id:'w',local:2,remote:3,base:1,revision:4,kind,candidates:[],fingerprint:kind});
beforeEach(async()=>{vi.stubGlobal('localStorage',{getItem:()=>null});vi.stubGlobal('navigator',{language:'en'});vi.stubGlobal('document',{documentElement:{lang:''}});await import('./i18n');await i18n.changeLanguage('en');});
afterEach(()=>vi.unstubAllGlobals());
describe('Bangumi two-way review',()=>{
 it('defaults only unambiguous changes and requires a choice for conflicts, missing records and first sync',()=>{
  expect(defaultBangumiAction(row('push'))).toBe('local');expect(defaultBangumiAction(row('pull'))).toBe('remote');expect(defaultBangumiAction(row('import'))).toBe('import');
  for(const kind of ['cleared','conflict','initial','missing','hidden','ignored','match','skipped'])expect(defaultBangumiAction(row(kind))).toBe('');
 });
 it('pre-selects only the suggested match and says so',()=>{
  const item={...row('match'),work_id:null,local:null,candidates:[{id:'w1',title:'One'},{id:'w2',title:'Two'}]};
  expect(defaultBangumiAction(item)).toBe('');expect(defaultBangumiAction({...item,suggested:'w2'})).toBe('match:w2');
  const html=renderToStaticMarkup(<BangumiRows rows={[{...item,suggested:'w2'}]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);
  expect(html).toContain('value="match:w2" selected=""');expect(html).toContain('pre-selected, check before applying');
  expect(renderToStaticMarkup(<BangumiRows rows={[{...item,suggested:'w2'}]} choices={{[item.fingerprint]:''}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>)).toContain('value="" selected=""');
 });
 it('keeps subject identity and snapshot in every reviewed action',()=>{expect(bangumiChoice(row('match'),'match:work-2')).toEqual({subject_id:1,fingerprint:'match',action:'match',work_id:'work-2'});expect(bangumiChoice(row('push'),'local').action).toBe('local');});
 it('shows both directions and escapes untrusted titles',()=>{const html=renderToStaticMarkup(<BangumiRows rows={[row('conflict')]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(html).toContain('Galroon → Bangumi');expect(html).toContain('Bangumi → Galroon');expect(html).toContain('Both sides changed');expect(html).toContain('A &lt;game&gt;');expect(html).not.toContain('<game>');expect(html).toContain('value="" selected=""');});
 it('offers a restore instead of a sync operation for hidden cards',()=>{const html=renderToStaticMarkup(<BangumiRows rows={[row('hidden')]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(html).toContain('Show and resume sync');expect(html).not.toContain('<select');});
 it('renders Traditional Chinese and keeps unknown local works as import candidates',async()=>{await i18n.changeLanguage('zh-TW');const item={...row('import'),work_id:null,local:null};const html=renderToStaticMarkup(<BangumiRows rows={[item]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(html).toContain('匯入為無本機資源的作品卡');expect(html).not.toContain('Galroon → Bangumi');});
});

it('shows cleared status with only remote or ignore choices',async()=>{
 await i18n.changeLanguage('zh-TW');const cleared={...row('cleared'),local:0,remote:1,base:1};const html=renderToStaticMarkup(<BangumiRows rows={[cleared]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);
 expect(html).toContain('本機已清除狀態；Bangumi 無法取消收藏，遠端保留');expect(html).toContain('value="remote"');expect(html).not.toContain('value="local"');expect(defaultBangumiAction(cleared)).toBe('');expect(bangumiStatus(0)).toBe('bgmNotCollected');
 const missing=renderToStaticMarkup(<BangumiRows rows={[{...cleared,kind:'missing',remote:null}]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(missing).not.toContain('value="local"');
});

it('labels a linked work without a play status as No status, not Not collected',async()=>{
 await i18n.changeLanguage('zh-TW');const pull={...row('pull'),local:0,remote:1,base:null};const html=renderToStaticMarkup(<BangumiRows rows={[pull]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);
 expect(html).toContain('Galroon: 無狀態 · Bangumi: 想玩');expect(bangumiLocalStatus(0)).toBe('none');expect(bangumiLocalStatus(null)).toBe('bgmNotCollected');expect(bangumiStatus(0)).toBe('bgmNotCollected');
});
