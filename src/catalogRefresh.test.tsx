import {describe,expect,it,vi} from 'vitest';
import {renderToStaticMarkup} from 'react-dom/server';
import {SourcePath} from './SourcePath';
const mock=vi.hoisted(()=>({api:vi.fn()}));vi.mock('./api',()=>({api:mock.api}));
import {readCatalogView} from './catalogRefresh';
describe('projected web view',()=>{
 it('renders a source without path and preserves desktop paths',()=>{expect(renderToStaticMarkup(<SourcePath/>)).toBe('');expect(renderToStaticMarkup(<SourcePath path={"C:\\Games"}/>)).toContain('C:\\Games');});
 it('loads web roots and jobs without requesting denied plans or quarantine',async()=>{const paths:string[]=[];mock.api.mockImplementation(async(path:string)=>{paths.push(path);if(path==='/access/me')return {role:'web',can_write:false};if(path==='/plans'||path==='/quarantine')throw new Error('Forbidden');if(path==='/roots')return [{id:'root',label:'Games',online:true}];return {};});const view=await readCatalogView(null);expect(view.roots[0].path).toBeUndefined();expect(view.plans).toEqual([]);expect(view.isolated).toEqual([]);expect(paths).toContain('/changes');expect(paths).not.toContain('/plans');expect(paths).not.toContain('/quarantine');});
 it('keeps desktop full data and reloads the currently opened work',async()=>{const paths:string[]=[];mock.api.mockImplementation(async(path:string)=>{paths.push(path);return path==='/access/me'?{role:'desktop',can_write:true}:{title:'Changed'};});const view=await readCatalogView('work');expect(paths).toEqual(expect.arrayContaining(['/plans','/quarantine','/works/work']));expect(view.work?.title).toBe('Changed');});
});
