import {test,expect} from '@playwright/test';
import {openApp,nav,T,openWork} from './support/app';
import {startCore,type Core} from './support/core';
let core:Core;
test.beforeEach(async()=>{core=await startCore({name:'vndb'});});
test.afterEach(async()=>{await core?.stop();});
test('VNDB review keeps status and rating directions independent',async({page})=>{
 await openApp(page,core);
 const account={user:{id:'u9',username:'測試帳號'},mode:'full',automatic:false,initialized:false,last_sync:null,error:''};
 const row=(field:string,kind:string,local:number,remote:number)=>({subject_id:1,field,title:'測試作品',work_id:'w',local,remote,base:field==='rating'?80:1,remote_rating:null,kind,candidates:[],fingerprint:field});
 let finishing=false;let state:any={account,running:false,preview:'',rows:[]},submitted:any;
 await page.route('**/api/vndb_sync**',async route=>{const path=new URL(route.request().url()).pathname;if(finishing&&route.request().method()==='GET'){state={account:{...account,initialized:true},running:false,preview:'test',rows:[]};finishing=false;}if(path.endsWith('/preview'))state={account,preview:'test',rows:[row('status','push',2,1),row('rating','conflict',85,90)]};if(path.endsWith('/apply')){submitted=route.request().postDataJSON();state={...state,running:true};finishing=true;}await route.fulfill({json:state});});
 await page.getByRole('button',{name:T('settings'),exact:true}).click();await page.getByRole('tab',{name:T('settings_account'),exact:true}).click();
 const panel=page.locator('.vndb-panel');await panel.getByRole('button',{name:T('vnsPreview')}).click();
 await expect(panel.getByLabel(T('vnsChooseFor',{title:'測試作品',field:T('vnsField_status')}))).toHaveValue('local');
 const rating=panel.getByLabel(T('vnsChooseFor',{title:'測試作品',field:T('vnsField_rating')}));await expect(rating).toHaveValue('');await expect(panel.getByText('Galroon: 8.5 · VNDB: 9.0')).toBeVisible();await rating.selectOption('remote');
 await page.evaluate(()=>window.scrollTo(0,0));await page.screenshot({path:'test-output/vndb-review.png',fullPage:true});await panel.getByRole('button',{name:T('vnsApply',{count:2})}).click();
 await expect.poll(()=>submitted?.choices.map((c:any)=>[c.field,c.action])).toEqual([['status','local'],['rating','remote']]);await expect(panel.getByText(T('vnsNoChanges'))).toBeVisible();
});
test('a resource-free card can save and clear a decimal personal rating',async({page})=>{
 const created=await core.api('/works',{title:'VNDB 評分作品'});const work=await core.api(`/works/${created.id}`);await openApp(page,core);await nav(page,'library');await openWork(page,work);
 const input=page.getByLabel(T('vnsMyRating'));await input.fill('8.7');await page.getByRole('button',{name:T('vnsSaveRating')}).click();await expect.poll(async()=>(await core.api(`/works/${work.id}`)).personal_rating).toBe(87);await expect(input).toHaveValue('8.7');
 await input.fill('');await page.getByRole('button',{name:T('vnsSaveRating')}).click();await expect.poll(async()=>(await core.api(`/works/${work.id}`)).personal_rating).toBeNull();await expect(page.getByLabel(T('vnsSubject'))).toBeVisible();
});
