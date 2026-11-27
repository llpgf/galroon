import {describe,it,expect,vi,beforeEach,afterEach} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import {PROVIDER_DEFAULTS,TranslationPanel} from './TranslationPanel';
import {TranslatedDescription} from './TranslatedDescription';

vi.mock('./api',()=>({
 api:vi.fn(),
 deleteApi:vi.fn(),
 readApi:vi.fn(),
}));

describe('LLM translation UI',()=>{
 beforeEach(async()=>{
  vi.stubGlobal('localStorage',{getItem:()=>null});
  vi.stubGlobal('navigator',{language:'zh-TW'});
  vi.stubGlobal('document',{documentElement:{lang:''}});
  const {default:i18n}=await import('./i18n');
  await i18n.changeLanguage('zh-TW');
 });
 afterEach(()=>vi.unstubAllGlobals());

 it('exposes official OpenAI, Gemini, and Claude compatible defaults',()=>{
  expect(PROVIDER_DEFAULTS.openai.baseUrl).toBe('https://api.openai.com/v1/');
  expect(PROVIDER_DEFAULTS.gemini.baseUrl).toBe('https://generativelanguage.googleapis.com/v1beta/');
  expect(PROVIDER_DEFAULTS.claude.baseUrl).toBe('https://api.anthropic.com/v1/');
 });

 it('renders TranslationPanel header and TranslatedDescription with spoiler masking',()=>{
  const panel=renderToStaticMarkup(<TranslationPanel/>);
  expect(panel).toContain('LLM 介紹翻譯');
  const hidden=renderToStaticMarkup(<TranslatedDescription kind="work" id="w1" sourceText="Hello [spoiler]secret[/spoiler]" spoilers={false} canWrite={true}/>);
  expect(hidden).toContain('Hello');
  expect(hidden).not.toContain('secret');
  const shown=renderToStaticMarkup(<TranslatedDescription kind="work" id="w1" sourceText="Hello [spoiler]secret[/spoiler]" spoilers={true} canWrite={true}/>);
  expect(shown).toContain('secret');
 });
});
