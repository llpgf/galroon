import {describe,expect,it} from 'vitest';
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
