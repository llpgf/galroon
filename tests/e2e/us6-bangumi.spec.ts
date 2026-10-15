// UI acceptance uses generated Bangumi responses; upstream HTTP and sync are tested in Rust.
import {test,expect} from '@playwright/test';
import {openApp,nav,T,openWork} from './support/app';
import {startCore,type Core} from './support/core';
let core:Core;
test.beforeEach(async()=>{core=await startCore({name:'bangumi'});});
test.afterEach(async()=>{await core?.stop();});

test('Bangumi review shows both directions, leaves conflicts unselected, and sends reviewed choices',async({page})=>{
 await openApp(page,core);
 const account={user:{id:9,username:'fixture',nickname:'測試帳號'},mode:'full',automatic:false,initialized:false,last_sync:null,error:''};
 const row=(id:number,kind:string,local:number|null,remote:number|null)=>({subject_id:id,title:`測試作品 ${id}`,work_id:local===null?null:`work-${id}`,local,remote,base:3,revision:1,kind,candidates:[],fingerprint:`row-${id}`});
 let state:any={account,preview:'',rows:[]};let submitted:any;
 await page.route('**/api/bangumi**',async route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/preview'))state={account,preview:'generated-preview',rows:[row(1,'push',2,3),row(2,'pull',3,4),row(3,'conflict',2,5),row(4,'import',null,2)]};
  if(path.endsWith('/apply')){submitted=route.request().postDataJSON();state={account:{...account,initialized:true},preview:'generated-preview',rows:[],results:submitted.choices.map((c:any)=>({subject_id:c.subject_id,ok:true}))};}
  await route.fulfill({json:state});
 });
 await page.getByRole('button',{name:T('settings'),exact:true}).click();await page.getByRole('tab',{name:T('settings_account'),exact:true}).click();
 await expect(page.getByRole('heading',{name:T('bgmTitle')})).toBeVisible();await page.getByRole('button',{name:T('bgmPreview')}).click();
 await expect(page.locator('.bgm-row')).toHaveCount(4);
 await expect(page.getByLabel(T('bgmAutomatic'),{exact:true})).toHaveCSS('width','16px');
 await expect(page.getByLabel(T('bgmChooseFor',{title:'測試作品 1'}))).toHaveValue('local');
 await expect(page.getByLabel(T('bgmChooseFor',{title:'測試作品 2'}))).toHaveValue('remote');
 const conflict=page.getByLabel(T('bgmChooseFor',{title:'測試作品 3'}));await expect(conflict).toHaveValue('');
 await conflict.selectOption('remote');await page.evaluate(()=>window.scrollTo(0,0));await page.screenshot({path:'test-output/bangumi-review.png',fullPage:true});
 await page.getByRole('button',{name:T('bgmApply',{count:4})}).click();
 await expect.poll(()=>submitted?.choices.map((c:any)=>[c.subject_id,c.action])).toEqual([[1,'local'],[2,'remote'],[3,'remote'],[4,'import']]);
 await expect(page.getByText(T('bgmNoChanges'))).toBeVisible();
});

test('a work without resources has a card and a Bangumi association entry',async({page})=>{
 const created=await core.api('/works',{title:'無本機資源的遊戲'});const work=await core.api(`/works/${created.id}`);
 await openApp(page,core);await nav(page,'library');await openWork(page,work);
 await expect(page.getByRole('heading',{name:T('bgmTitle')})).toBeVisible();
 await expect(page.locator('#work-files').getByText(T('availability_no_resources'),{exact:true})).toBeVisible();
 await expect(page.getByLabel(T('bgmSubject'))).toBeVisible();
});
