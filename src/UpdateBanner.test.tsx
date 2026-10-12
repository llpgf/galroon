import {renderToStaticMarkup} from 'react-dom/server';
import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
vi.mock('./api',()=>({api:vi.fn()}));
import i18n from 'i18next';
import {APP_VERSION} from './version';
import {UpdateBanner} from './UpdateBanner';
import {AboutPanel} from './AboutPanel';
import {SettingsLayout} from './SettingsLayout';
import type {UpdateState} from './updates';
const state:UpdateState={current:APP_VERSION,install_kind:'docker',enabled:true,checking:false,last_checked:null,last_error:null,update_available:true,latest:{version:'9.0.0',name:'New release',url:'https://github.com/llpgf/galroon/releases/tag/v9.0.0',notes:'<img src=x onerror=alert(1)>\nPlain text',published_at:null}};
beforeEach(async()=>{vi.stubGlobal('localStorage',{getItem:()=>null});vi.stubGlobal('navigator',{language:'en'});vi.stubGlobal('document',{documentElement:{lang:''}});await import('./i18n');await i18n.changeLanguage('en');});
afterEach(()=>vi.unstubAllGlobals());
describe('release notice UI',()=>{
 it('escapes release notes, links normally and explains Docker updates',async()=>{await i18n.changeLanguage('en');const html=renderToStaticMarkup(<UpdateBanner state={state} coreVersion={APP_VERSION}/>);expect(html).toContain('&lt;img');expect(html).not.toContain('<img');expect(html).toContain('container image');expect(html).toContain('target="_blank" rel="noreferrer"');expect(html).toContain('<details>');expect(html).toContain('Skip this version');});
 it('shows version mismatch without GitHub and even when the release was skipped',()=>{vi.stubGlobal('localStorage',{getItem:()=>state.latest!.version});for(const update of [null,state]){const html=renderToStaticMarkup(<UpdateBanner state={update} coreVersion="1.2.3"/>);expect(html).toContain('different versions; update both');expect(html).not.toContain('Open download page');}expect(renderToStaticMarkup(<UpdateBanner state={null} coreVersion={APP_VERSION}/>)).toBe('');});
 it('offers update controls only to writable roles and always includes About',()=>{const updates={state,error:'',busy:false,check:vi.fn(),setEnabled:vi.fn()};const web=renderToStaticMarkup(<AboutPanel updates={updates} coreVersion={APP_VERSION} canWrite={false}/>);expect(web).toContain('Core version');expect(web).toContain('Client version');expect(web).toContain('Never');expect(web).not.toContain('role="switch"');expect(web).not.toContain('Check now');const owner=renderToStaticMarkup(<AboutPanel updates={updates} canWrite/>);expect(owner).toContain('role="switch"');expect(owner).toContain('Check now');expect(renderToStaticMarkup(<SettingsLayout appearance={<p/>} collection={null} account={null} backup={null} about={<p>About</p>}/>)).toContain('settings-tab-about');});
 it('includes Traditional Chinese update strings',async()=>{await i18n.changeLanguage('zh-TW');expect(renderToStaticMarkup(<UpdateBanner state={state}/>)).toContain('開啟下載頁面');await i18n.changeLanguage('en');});
});
