import {describe,expect,it,vi} from 'vitest';
import type {TFunction} from 'i18next';
import {coreText} from './coreText';
const t=((key:string,options?:{name?:string;defaultValue?:string})=>key==='coreText_otherSourceName'?`other:${options?.name}`:key==='coreText_nonUtf8Skipped'?`skipped:${options?.name}`:key) as unknown as TFunction;
describe('coreText',()=>{
 it('maps known Core sentences to translation keys',()=>{expect(coreText(t,'No VNDB candidate found.')).toBe('coreText_noCandidate');expect(coreText(t,'Scan complete')).toBe('coreText_scanComplete');expect(coreText(t,'Scan completed with errors. Some folders could not be read.')).toBe('coreText_scanErrors');});
 it('keeps the variable part of prefixed reasons',()=>{expect(coreText(t,'A different source name needs review: Fate/stay night')).toBe('other:Fate/stay night');});
 it('translates skipped non-UTF-8 names and preserves the displayable path',()=>{expect(coreText(t,'File name is not valid UTF-8 and was skipped: /source/遊戲/invalid-�.bin')).toBe('skipped:/source/遊戲/invalid-�.bin');});
 it('names VNDB outages and unchecked resources honestly',()=>{expect(coreText(t,'Not checked against VNDB yet.')).toBe('coreText_notChecked');expect(coreText(t,'VNDB could not be reached within 8 seconds. Try again later.')).toBe('coreText_vndbUnreachable');expect(coreText(t,'Collection or filters changed. Reload before continuing.')).toBe('coreText_collectionChanged');});
 it('shows unknown text as Core wrote it',()=>{expect(coreText(t,'Disk full')).toBe('Disk full');expect(coreText(t,'')).toBe('');expect(coreText(t,null)).toBe('');});
});

it('shows the shared stale revision message in English and Traditional Chinese',async()=>{
 vi.stubGlobal('localStorage',{getItem:()=>null,setItem:()=>{}});vi.stubGlobal('navigator',{language:'en'});vi.stubGlobal('document',{documentElement:{lang:''}});
 const {default:i18n}=await import('./i18n');const {ApiRequestError}=await import('./api');
 for(const [language,message] of [['en','This item was changed on another device. Reload it.'],['zh-TW','這筆資料已被其他裝置修改，請重新載入']]){
  await i18n.changeLanguage(language);
  for(const raw of ['This work changed. Reload before editing.','Edition changed. Reload before saving.','Collection name changed. Refresh before saving.','Relationship decisions changed. Reload first.','List changed. Reload from the first page.']){expect(new ApiRequestError(raw,400).message).toBe(message);expect(coreText(i18n.t.bind(i18n),raw)).toBe(message);}
 }
 expect(new ApiRequestError('Disk full',400).message).toBe('Disk full');await i18n.changeLanguage('en');vi.unstubAllGlobals();
});
