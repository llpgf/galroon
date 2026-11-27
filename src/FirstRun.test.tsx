import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import {isValidElement,type ReactElement,type ReactNode} from 'react';
const hooks=vi.hoisted(()=>({
 values:[] as unknown[],
 cursor:0,
 effects:new Map<number,{deps?:unknown[];cleanup?:(()=>void)|void}>(),
 pending:[] as (()=>void)[],
}));
const mockApi=vi.hoisted(()=>({
 api:vi.fn(),
 coreHealth:vi.fn(),
 bytes:vi.fn((n:number)=>`${n} B`),
 ApiRequestError:class extends Error{constructor(message:string,public status:number,public code?:string){super(message);}},
}));
vi.mock('react',async original=>({
 ...await original<typeof import('react')>(),
 useState:(initial:unknown)=>{
  const i=hooks.cursor++;
  if(!(i in hooks.values))hooks.values[i]=typeof initial==='function'?(initial as ()=>unknown)():initial;
  return [hooks.values[i],(value:unknown)=>{hooks.values[i]=typeof value==='function'?(value as (prev:unknown)=>unknown)(hooks.values[i]):value;}];
 },
 useEffect:(effect:()=>void|(()=>void),deps?:unknown[])=>{
  const i=hooks.cursor++;
  const prev=hooks.effects.get(i);
  const changed=!prev||!deps||!prev.deps||deps.length!==prev.deps.length||deps.some((d,idx)=>!Object.is(d,prev.deps![idx]));
  if(changed){
   hooks.pending.push(()=>{
    if(typeof prev?.cleanup==='function')prev.cleanup();
    const cleanup=effect();
    hooks.effects.set(i,{deps,cleanup});
   });
  }
 },
}));
vi.mock('react-i18next',()=>({useTranslation:()=>({t:(key:string,opts?:Record<string,unknown>)=>opts?`${key}:${JSON.stringify(opts)}`:key})}));
vi.mock('./api',()=>mockApi);
import {FirstRun,type RestoreCandidatesResponse,type RestoreInspectResponse} from './FirstRun';
import {ApiRequestError} from './api';
type Element=ReactElement<Record<string,any>>;
function nodes(node:ReactNode):Element[]{
 if(Array.isArray(node))return node.flatMap(nodes);
 if(!isValidElement(node))return [];
 const el=node as Element;
 return [el,...nodes(el.props.children)];
}
function textOf(node:ReactNode):string{
 if(node===null||node===undefined||typeof node==='boolean')return '';
 if(typeof node==='string'||typeof node==='number')return String(node);
 if(Array.isArray(node))return node.map(textOf).join('');
 if(isValidElement(node))return textOf((node as Element).props.children);
 return '';
}
function resetHooks(){
 for(const entry of hooks.effects.values())if(typeof entry.cleanup==='function')entry.cleanup();
 hooks.values=[];hooks.cursor=0;hooks.effects.clear();hooks.pending=[];
}
function flushEffects(){
 const queue=hooks.pending.splice(0);
 for(const fn of queue)fn();
}
function render(props:Parameters<typeof FirstRun>[0]){
 hooks.cursor=0;
 const tree=nodes(FirstRun(props));
 flushEffects();
 return tree;
}
async function settle(){for(let i=0;i<10;i++)await Promise.resolve();}
const sampleCandidates:RestoreCandidatesResponse={
 searched:['/backups'],
 candidates:[{
  path:'/backups/Galroon-backup-2026',
  name:'Galroon-backup-2026',
  created:1760587200,
  version:4,
  mode:'full',
  origin:'scheduled',
  bytes:1024,
  sha256:'deadbeef',
  schema:52,
  has_artwork:true,
  artwork_bytes:2048,
  library:{id:'lib-1',name:'Main Archive',revision:3},
  spki_fingerprint:'sha256:SPKI-FULL',
  core_version:'0.1.0',
 }],
};
const fullInspect:RestoreInspectResponse={
 path:'/backups/Galroon-backup-2026',
 manifest:{version:4},
 mode:'full',
 origin:'scheduled',
 sha256:'deadbeef',
 schema:52,
 works:42,
 resources:50,
 files:300,
 roots:2,
 spki_fingerprint:'sha256:SPKI-FULL',
 device_id:'dev-orig',
 requires_original_offline:true,
 network_address:'192.168.50.154',
 network_address_local:false,
};
describe('FirstRun (B5)',()=>{
 beforeEach(()=>{resetHooks();vi.clearAllMocks();vi.useFakeTimers();});
 afterEach(()=>{resetHooks();vi.clearAllTimers();vi.useRealTimers();});

 it('shows both choices on a fresh Core and allows creating a new collection',async()=>{
  const onComplete=vi.fn();
  mockApi.api.mockImplementation(async(path:string)=>{
   if(path==='/setup/restore/candidates')return sampleCandidates;
   if(path==='/access/setup')return {configured:true};
   return {};
  });
  render({onComplete});
  await settle();
  let tree=render({onComplete});
  expect(tree.some(el=>el.type==='button'&&textOf(el)==='firstRunCreateAction')).toBe(true);
  expect(tree.some(el=>el.type==='button'&&textOf(el)==='firstRunRestoreAction')).toBe(true);

  tree.find(el=>el.type==='button'&&textOf(el)==='firstRunCreateAction')!.props.onClick();
  tree=render({onComplete});
  const pwLabel=tree.find(el=>el.type==='label'&&textOf(el).includes('ownerPassword'))!;
  const confirmLabel=tree.find(el=>el.type==='label'&&textOf(el).includes('confirmOwnerPassword'))!;
  nodes(pwLabel).find(el=>el.type==='input')!.props.onChange({target:{value:'strong-owner-password'}});
  nodes(confirmLabel).find(el=>el.type==='input')!.props.onChange({target:{value:'strong-owner-password'}});
  tree=render({onComplete});
  tree.find(el=>el.type==='form'&&el.props['aria-label']==='setupOwner')!.props.onSubmit({preventDefault:vi.fn()});
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/access/setup',{password:'strong-owner-password'});
  expect(onComplete).toHaveBeenCalledTimes(1);
 });

 it('falls back to owner setup only when Core returns 409 core_not_fresh',async()=>{
  mockApi.api.mockRejectedValueOnce(new ApiRequestError('Core is not fresh',409,'core_not_fresh'));
  render({onComplete:vi.fn()});
  await settle();
  const tree=render({onComplete:vi.fn()});
  expect(tree.some(el=>el.type==='form'&&el.props['aria-label']==='setupOwner')).toBe(true);
  expect(tree.some(el=>el.type==='button'&&textOf(el)==='firstRunRestoreAction')).toBe(false);
  expect(tree.some(el=>textOf(el).includes('firstRunNotFreshNote'))).toBe(true);
 });

 it('requires original-offline confirmation for full restore, polls /api/health, and shows lan_disabled & full reconnect guidance',async()=>{
  const onReconnect=vi.fn();
  mockApi.api.mockImplementation(async(path:string)=>{
   if(path==='/setup/restore/inspect')return fullInspect;
   if(path==='/setup/restore')return {
    status:'relaunching',mode:'full',collection:'/data/collections/c1',relaunch:true,
    lan_disabled:true,warning:'Update LAN address in settings.',replaced_identity:'/data/replaced-1',requires_new_session:true,
   };
   return {};
  });
  mockApi.coreHealth.mockResolvedValueOnce(false).mockResolvedValueOnce(true);

  const props={initialCandidates:sampleCandidates,onComplete:vi.fn(),onReconnect,pollIntervalMs:500};
  let tree=render(props);
  tree.find(el=>el.type==='button'&&textOf(el)==='firstRunRestoreAction')!.props.onClick();
  tree=render(props);
  expect(tree.some(el=>textOf(el).includes('Main Archive'))).toBe(true);

  // Inspect candidate
  tree.find(el=>el.type==='button'&&textOf(el)==='firstRunSelectCandidate')!.props.onClick();
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/setup/restore/inspect',{path:'/backups/Galroon-backup-2026'});
  tree=render(props);
  const preview=tree.find(el=>el.props.role==='region'&&el.props['aria-label']==='firstRunInspectTitle')!;
  expect(textOf(preview)).toContain('42');
  expect(textOf(preview)).toContain('50');
  expect(textOf(preview)).toContain('Schema 52');
  expect(textOf(preview)).toContain('sha256:SPKI-FULL');
  expect(textOf(preview)).toContain('firstRunNetworkNotLocal');

  // Restore button is disabled until mandatory original-offline checkbox is checked
  let restoreBtn=nodes(preview).find(el=>el.type==='button'&&textOf(el)==='firstRunStartRestore')!;
  expect(restoreBtn.props.disabled).toBe(true);
  const checkbox=nodes(preview).find(el=>el.type==='input'&&el.props.type==='checkbox')!;
  checkbox.props.onChange({target:{checked:true}});
  tree=render(props);
  restoreBtn=tree.find(el=>el.type==='button'&&textOf(el)==='firstRunStartRestore')!;
  expect(restoreBtn.props.disabled).toBe(false);

  // Trigger restore -> enters restarting state and polls /api/health
  restoreBtn.props.onClick();
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/setup/restore',{path:'/backups/Galroon-backup-2026',sha256:'deadbeef',original_offline:true});
  tree=render(props);
  expect(tree.some(el=>textOf(el).includes('firstRunRestartingTitle'))).toBe(true);
  expect(tree.some(el=>textOf(el).includes('Update LAN address in settings.'))).toBe(true);

  // Poll 1: health returns false (Core shutting down)
  await vi.advanceTimersByTimeAsync(500);
  await settle();
  expect(mockApi.coreHealth).toHaveBeenCalledTimes(1);
  tree=render(props);
  expect(tree.some(el=>textOf(el).includes('firstRunRestartingTitle'))).toBe(true);

  // Poll 2: health returns true (Core relaunched)
  await vi.advanceTimersByTimeAsync(500);
  await settle();
  expect(mockApi.coreHealth).toHaveBeenCalledTimes(2);
  tree=render(props);
  expect(tree.some(el=>textOf(el).includes('firstRunRestartedTitle'))).toBe(true);
  expect(tree.some(el=>textOf(el).includes('firstRunReconnectFull'))).toBe(true);
  tree.find(el=>el.type==='button'&&textOf(el)==='firstRunReload')!.props.onClick();
  expect(onReconnect).toHaveBeenCalledTimes(1);
 });

 it('supports manual path inspect and portable restore without original-offline checkbox',async()=>{
  mockApi.api.mockImplementation(async(path:string)=>{
   if(path==='/setup/restore/inspect')return {
    path:'/custom/portable-backup',manifest:{version:4},mode:'portable',origin:'manual',
    sha256:'cafebabe',schema:52,works:5,resources:7,files:20,roots:1,
    spki_fingerprint:null,device_id:null,requires_original_offline:false,network_address:null,network_address_local:null,
   };
   if(path==='/setup/restore')return {
    status:'relaunching',mode:'portable',collection:'/data/collections/c2',relaunch:true,
    lan_disabled:false,warning:null,replaced_identity:null,requires_new_session:true,
   };
   return {};
  });
  mockApi.coreHealth.mockResolvedValue(true);

  const props={initialCandidates:{candidates:[],searched:['/backups']},onComplete:vi.fn(),pollIntervalMs:200};
  let tree=render(props);
  tree.find(el=>el.type==='button'&&textOf(el)==='firstRunRestoreAction')!.props.onClick();
  tree=render(props);
  const pathLabel=tree.find(el=>el.type==='label'&&textOf(el).includes('firstRunBackupPath'))!;
  nodes(pathLabel).find(el=>el.type==='input')!.props.onChange({target:{value:' /custom/portable-backup '}});
  tree=render(props);
  tree.find(el=>el.type==='form'&&el.props['aria-label']==='inspectBackup')!.props.onSubmit({preventDefault:vi.fn()});
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/setup/restore/inspect',{path:'/custom/portable-backup'});

  tree=render(props);
  const restoreBtn=tree.find(el=>el.type==='button'&&textOf(el)==='firstRunStartRestore')!;
  expect(restoreBtn.props.disabled).toBe(false);
  expect(tree.some(el=>el.type==='input'&&el.props.type==='checkbox')).toBe(false);
  restoreBtn.props.onClick();
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/setup/restore',{path:'/custom/portable-backup',sha256:'cafebabe',original_offline:false});
  render(props);
  await vi.advanceTimersByTimeAsync(400);
  await settle();
  tree=render(props);
  expect(tree.some(el=>textOf(el).includes('firstRunRestartedTitle'))).toBe(true);
  expect(tree.some(el=>textOf(el).includes('firstRunReconnectPortable'))).toBe(true);
 });
});
