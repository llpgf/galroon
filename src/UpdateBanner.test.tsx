import {renderToStaticMarkup} from 'react-dom/server';
import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
vi.mock('./api',()=>({api:vi.fn(),bytes:(n:number)=>`${n} B`,connect:vi.fn(),coreHealth:vi.fn(),isDesktop:false}));
import i18n from 'i18next';
import {APP_VERSION} from './version';
import {UpdateBanner} from './UpdateBanner';
import {AboutPanel} from './AboutPanel';
import {SettingsLayout} from './SettingsLayout';
import type {UpdateState} from './updates';
import type {useUpdates} from './useUpdates';
const state:UpdateState={current:APP_VERSION,install_kind:'docker',can_self_update:false,enabled:true,checking:false,last_checked:null,last_error:null,update_available:true,latest:{version:'9.0.0',name:'New release',url:'https://github.com/llpgf/galroon/releases/tag/v9.0.0',notes:'<img src=x onerror=alert(1)>\nPlain text',published_at:null}};
const makeUpdates=(overrides:Partial<ReturnType<typeof useUpdates>>={}):ReturnType<typeof useUpdates>=>({
 state,error:'',busy:false,applying:false,progress:{phase:'idle'},canApply:false,canUpdateCore:false,canUpdateDesktop:false,needsForceConfirm:false,check:vi.fn(),setEnabled:vi.fn(),apply:vi.fn(),...overrides
});
beforeEach(async()=>{vi.stubGlobal('localStorage',{getItem:()=>null});vi.stubGlobal('navigator',{language:'en'});vi.stubGlobal('document',{documentElement:{lang:''}});await import('./i18n');await i18n.changeLanguage('en');});
afterEach(()=>vi.unstubAllGlobals());
describe('release notice UI',()=>{
 it('escapes release notes, links normally and explains Docker updates',async()=>{await i18n.changeLanguage('en');const html=renderToStaticMarkup(<UpdateBanner state={state} coreVersion={APP_VERSION}/>);expect(html).toContain('&lt;img');expect(html).not.toContain('<img');expect(html).toContain('container image');expect(html).toContain('target="_blank" rel="noreferrer"');expect(html).toContain('<details>');expect(html).toContain('Skip this version');});
 it('shows version mismatch without GitHub and even when the release was skipped',()=>{vi.stubGlobal('localStorage',{getItem:()=>state.latest!.version});for(const update of [null,state]){const html=renderToStaticMarkup(<UpdateBanner state={update} coreVersion="1.2.3"/>);expect(html).toContain('different versions; update both');expect(html).not.toContain('Open download page');}expect(renderToStaticMarkup(<UpdateBanner state={null} coreVersion={APP_VERSION}/>)).toBe('');});
 it('offers update controls only to writable roles and always includes About',()=>{const updates=makeUpdates();const web=renderToStaticMarkup(<AboutPanel updates={updates} coreVersion={APP_VERSION} canWrite={false}/>);expect(web).toContain('Core version');expect(web).toContain('Client version');expect(web).toContain('Never');expect(web).not.toContain('role="switch"');expect(web).not.toContain('Check now');const owner=renderToStaticMarkup(<AboutPanel updates={updates} canWrite/>);expect(owner).toContain('role="switch"');expect(owner).toContain('Check now');expect(renderToStaticMarkup(<SettingsLayout appearance={<p/>} collection={null} account={null} backup={null} about={<p>About</p>}/>)).toContain('settings-tab-about');});
 it('renders coordinated Core & Desktop update button, progress bar and force-pause option',async()=>{
  await i18n.changeLanguage('en');
  const updates=makeUpdates({
   state:{...state,can_self_update:true},
   canApply:true,canUpdateCore:true,canUpdateDesktop:true,
   applying:true,progress:{phase:'core_downloading',targetVersion:'9.0.0',downloaded:512,total:1024}
  });
  const html=renderToStaticMarkup(<UpdateBanner state={updates.state} coreVersion={APP_VERSION} canWrite updates={updates}/>);
  expect(html).toContain('Updating…');
  expect(html).toContain('Step 1/2');
  expect(html).toContain('Downloading Core 9.0.0');
  expect(html).toContain('512 B / 1024 B (50%)');
  expect(html).toContain('<progress');
  const blocked=makeUpdates({state:{...state,can_self_update:true},canApply:true,canUpdateCore:true,error:'Active background tasks are running.',needsForceConfirm:true});
  const blockedHtml=renderToStaticMarkup(<AboutPanel updates={blocked} coreVersion={APP_VERSION} canWrite/>);
  expect(blockedHtml).toContain('Pause active tasks and update');
  expect(blockedHtml).toContain('update-alert warn');
  expect(blockedHtml).toContain('background task');
 });
 it('includes Traditional Chinese update strings',async()=>{
  await i18n.changeLanguage('zh-TW');
  expect(renderToStaticMarkup(<UpdateBanner state={state}/>)).toContain('開啟下載頁面');
  const both=makeUpdates({state:{...state,can_self_update:true},canApply:true,canUpdateCore:true,canUpdateDesktop:true});
  expect(renderToStaticMarkup(<UpdateBanner state={both.state} coreVersion={APP_VERSION} canWrite updates={both}/>)).toContain('同時更新 Core 與桌面版');
  await i18n.changeLanguage('en');
 });
});
