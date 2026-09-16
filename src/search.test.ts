import {describe,it,expect} from 'vitest';
import {fuzzyIncludes,releaseMonth} from './search';
describe('collection search and dates',()=>{
 it('finds title punctuation, width variants, word order and small typos',()=>{
  for(const query of ['steinsgate','ＳＴＥＩＮＳ gate','stiens gate','gate steins'])expect(fuzzyIncludes('STEINS;GATE NITRO PLUS',query)).toBe(true);
  expect(fuzzyIncludes('マガルミナ Purple software','マガル')).toBe(true);
  expect(fuzzyIncludes('Café Stella','cafe')).toBe(true);
 });
 it('requires every query term and avoids fuzzy matching tiny terms',()=>{
  expect(fuzzyIncludes('STEINS;GATE','steins clannad')).toBe(false);
  expect(fuzzyIncludes('Key','kay')).toBe(false);
  expect(fuzzyIncludes('CLANNAD','clanxad')).toBe(true);
 });
 it('shows known months without inventing missing dates',()=>{
  expect(releaseMonth('2026-02-27')).toBe('2026-02');expect(releaseMonth('2026-99-99')).toBe('2026');expect(releaseMonth('2026')).toBe('2026');expect(releaseMonth('TBA')).toBe('—');
 });
});
