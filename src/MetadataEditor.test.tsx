import {renderToStaticMarkup} from 'react-dom/server';
import {beforeEach,afterEach,describe,expect,it,vi} from 'vitest';
vi.mock('./api',()=>({api:vi.fn(()=>new Promise(()=>{})),artworkBlob:vi.fn()}));
import i18n from 'i18next';
import {MetadataEditor} from './MetadataEditor';
import type {Work} from './api';
const work:Work={id:'w1',title:'SUMMER-田舎の性活-',original_title:'SUMMER-田舎の性活-',vndb_id:null,description:'DLsite RJ351325',cover:'',developer:'ディーゼルマイン',released:'2021-12-24',tags:[],aliases:[],status:'none',favorite:false,notes:'',revision:1,resources:1};
beforeEach(async()=>{vi.stubGlobal('localStorage',{getItem:()=>null});vi.stubGlobal('navigator',{language:'en'});vi.stubGlobal('document',{documentElement:{lang:''}});await import('./i18n');await i18n.changeLanguage('en');});
afterEach(()=>vi.unstubAllGlobals());
describe('Roon-style MetadataEditor',()=>{
 it('renders the 3-tab Roon-style editor shell and localized Traditional Chinese labels',async()=>{
  const enHtml=renderToStaticMarkup(<MetadataEditor work={work} onClose={vi.fn()} onSaved={vi.fn()}/>);
  expect(enHtml).toContain('Identify &amp; sources');
  expect(enHtml).toContain('Metadata preferences');
  expect(enHtml).toContain('Edit work');
  expect(enHtml).toContain('Revert edits…');
  await i18n.changeLanguage('zh-TW');
  const zhHtml=renderToStaticMarkup(<MetadataEditor work={work} onClose={vi.fn()} onSaved={vi.fn()}/>);
  expect(zhHtml).toContain('辨識與來源');
  expect(zhHtml).toContain('詮釋資料偏好設定');
  expect(zhHtml).toContain('編輯作品');
  expect(zhHtml).toContain('還原編輯…');
  await i18n.changeLanguage('en');
 });
});
