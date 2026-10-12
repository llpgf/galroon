import {renderToStaticMarkup} from 'react-dom/server';
import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import i18n from 'i18next';
import {SuggestionActions,SuggestionCard} from './Suggestions';
import type {MatchSuggestion} from './api';
const suggestion:MatchSuggestion={id:'s',resource_id:'r',resource_revision:3,works:[{vndb_id:'v1',title:'First work'},{vndb_id:'v2',title:'Second work'}],query:'First',rationale:'<script>untrusted rationale</script>',source:'mcp:test',verified:true,state:'open',created:1,updated:1};
const actions={verified:[suggestion],busy:false,error:'',loadError:'',results:[],confirm:vi.fn(),dismiss:vi.fn(),batch:vi.fn(),reload:vi.fn()};
beforeEach(async()=>{vi.stubGlobal('localStorage',{getItem:()=>null});vi.stubGlobal('navigator',{language:'en'});vi.stubGlobal('document',{documentElement:{lang:''}});await import('./i18n');await i18n.changeLanguage('en');});
afterEach(()=>vi.unstubAllGlobals());
describe('suggestion review',()=>{
 it('shows all proposed works and escaped rationale collapsed by default',()=>{const html=renderToStaticMarkup(<SuggestionCard suggestion={suggestion} canWrite actions={actions}/>);expect(html).toContain('First work');expect(html).toContain('(v2)');expect(html).toContain('AI suggestion');expect(html).toContain('Verified');expect(html).toContain('<details>');expect(html).not.toContain('<details open');expect(html).toContain('&lt;script&gt;');expect(html).not.toContain('<script>');});
 it('keeps web suggestion details readable and omits all write controls',()=>{const html=renderToStaticMarkup(<><SuggestionCard suggestion={suggestion} canWrite={false} actions={actions}/><SuggestionActions canWrite={false} actions={actions}/></>);expect(html).toContain('First work');expect(html).not.toContain('<button');});
 it('only offers a batch when verified suggestions exist and labels unverified suggestions',()=>{expect(renderToStaticMarkup(<SuggestionActions canWrite actions={{...actions,verified:[]}}/>)).not.toContain('<button');expect(renderToStaticMarkup(<SuggestionActions canWrite actions={actions}/>)).toContain('Confirm all verified');expect(renderToStaticMarkup(<SuggestionCard suggestion={{...suggestion,verified:false}} canWrite actions={actions}/>)).toContain('Unverified');});
 it('shows each batch result, including failures, and translates zh-TW',async()=>{const html=renderToStaticMarkup(<SuggestionActions canWrite actions={{...actions,results:[{suggestion,result:{id:'s',state:'confirmed',resource:{id:'r',revision:4}}},{suggestion:{...suggestion,id:'failed'},result:{id:'failed',confirmed:false,error:'Changed'}}]}}/>);expect(html).toContain('Confirmed');expect(html).toContain('Not confirmed: Changed');await i18n.changeLanguage('zh-TW');const zh=renderToStaticMarkup(<SuggestionCard suggestion={suggestion} canWrite actions={actions}/>);expect(zh).toContain('AI 建議');expect(zh).toContain('已驗證');expect(zh).toContain('確認');expect(zh).toContain('略過');});
});
