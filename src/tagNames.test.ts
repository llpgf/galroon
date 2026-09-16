import {describe,it,expect} from 'vitest';
import {tagNameKey,similarTagNames} from './tagNames';
describe('tag naming guidance',()=>{
 it('warns about compatibility glyphs without treating them as duplicates',()=>{
  expect(similarTagNames('ABC',[{id:'1',name:'ＡＢＣ'}])).toHaveLength(1);
  expect(tagNameKey('ABC')).not.toBe(tagNameKey('ＡＢＣ'));
 });
 it('leaves canonical duplicate enforcement to Core and excludes the edited identity',()=>{
  expect(similarTagNames('Café',[{id:'1',name:'CAFE\u0301'}])).toEqual([]);
  expect(similarTagNames('ABC',[{id:'1',name:'ＡＢＣ'}],'1')).toEqual([]);
  expect(similarTagNames('',[{id:'1',name:'ＡＢＣ'}])).toEqual([]);
 });
 it('warns about repeated spaces but preserves their distinction',()=>{
  expect(similarTagNames('A B',[{id:'1',name:'A  B'}])).toHaveLength(1);
  expect(tagNameKey('A B')).not.toBe(tagNameKey('A  B'));
  expect(similarTagNames('Drama',[{id:'1',name:'Comedy'}])).toEqual([]);
 });
});
