import {beforeEach,afterEach,describe,it,expect,vi} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import i18n from 'i18next';
import {VndbRows} from './VndbPanel';
import {vndbChoice,vndbValue,defaultVndbAction,type VndbRow} from './vndb';
vi.mock('./api',()=>({api:vi.fn(),readApi:vi.fn()}));
const row=(kind:string,field:VndbRow['field']='rating'):VndbRow=>({subject_id:1,field,title:'A <game>',work_id:'w',local:85,remote:90,base:80,remote_rating:null,kind,candidates:[],fingerprint:kind+field});
beforeEach(async()=>{vi.stubGlobal('localStorage',{getItem:()=>null});vi.stubGlobal('navigator',{language:'en'});vi.stubGlobal('document',{documentElement:{lang:''}});await import('./i18n');await i18n.changeLanguage('en');});
afterEach(()=>vi.unstubAllGlobals());
describe('VNDB two-way review',()=>{
 it('requires explicit decisions for both rating and status conflicts',()=>{for(const field of ['rating','status'] as const){expect(defaultVndbAction(row('push',field))).toBe('local');expect(defaultVndbAction(row('pull',field))).toBe('remote');for(const kind of ['conflict','initial','missing','unsupported','hidden','ignored','match'])expect(defaultVndbAction(row(kind,field))).toBe('');}});
 it('keeps field identity in reviewed choices',()=>{expect(vndbChoice(row('push'),'local')).toEqual({subject_id:1,field:'rating',fingerprint:'pushrating',action:'local',work_id:null});});
 it('shows decimal ratings, unrated and independent controls for the same VN',()=>{const html=renderToStaticMarkup(<VndbRows rows={[row('conflict'),{...row('conflict','status'),local:1,remote:2}]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(html).toContain('8.5');expect(html).toContain('9.0');expect(html).toContain('Galroon → VNDB');expect(html).toContain('VNDB → Galroon');expect(html.match(/<select /g)).toHaveLength(2);expect(html).toContain('A &lt;game&gt;');expect(vndbValue('rating',0,k=>k)).toBe('vnsUnrated');});
 it('does not offer ambiguous remote status as a valid pull',()=>{const html=renderToStaticMarkup(<VndbRows rows={[{...row('unsupported','status'),remote:-1}]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(html).not.toContain('value="remote"');expect(html).toContain('value="local"');});
 it('renders import rating and Traditional Chinese',async()=>{await i18n.changeLanguage('zh-TW');const html=renderToStaticMarkup(<VndbRows rows={[{...row('import','entry'),work_id:null,local:null,remote:2,remote_rating:87}]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(html).toContain('8.7');expect(html).toContain('匯入為無本機資源的作品卡');});
});

it('shows zero as no status and allows pulling it',async()=>{
 expect(vndbValue('status',0,k=>k)).toBe('none');expect(vndbValue('status',-1,k=>k)).toBe('vnsUnknownStatus');await i18n.changeLanguage('zh-TW');const html=renderToStaticMarkup(<VndbRows rows={[{...row('pull','status'),local:1,remote:0}]} choices={{}} onChoice={()=>{}} onRestore={()=>{}} busy={false}/>);expect(html).toContain('無狀態');expect(html).toContain('value="remote" selected=""');
});
