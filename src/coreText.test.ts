import {describe,expect,it} from 'vitest';
import type {TFunction} from 'i18next';
import {coreText} from './coreText';
const t=((key:string,options?:{name?:string;defaultValue?:string})=>key==='coreText_otherSourceName'?`other:${options?.name}`:key) as unknown as TFunction;
describe('coreText',()=>{
 it('maps known Core sentences to translation keys',()=>{expect(coreText(t,'No VNDB candidate found.')).toBe('coreText_noCandidate');expect(coreText(t,'Scan complete')).toBe('coreText_scanComplete');});
 it('keeps the variable part of prefixed reasons',()=>{expect(coreText(t,'A different source name needs review: Fate/stay night')).toBe('other:Fate/stay night');});
 it('shows unknown text as Core wrote it',()=>{expect(coreText(t,'Disk full')).toBe('Disk full');expect(coreText(t,'')).toBe('');expect(coreText(t,null)).toBe('');});
});
