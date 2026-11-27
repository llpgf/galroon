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
 coreUrl:vi.fn(()=>'http://127.0.0.1:14800'),
 folder:vi.fn(),
 isLocalDesktop:vi.fn(()=>true),
 collectionStorageKey:vi.fn((name:string)=>`galroon.test.${name}`),
 bytes:vi.fn((n:number)=>`${n} B`),
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
vi.mock('./AcquireSummary',()=>({AcquireSummary:()=>null}));
import {AccessPanel} from './AccessPanel';
import {AcquirePanel} from './AcquirePanel';
import {SourceRemap} from './SourceRemap';
import {DuplicatePanel} from './DuplicatePanel';
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
function render<P>(Component:(props:P)=>ReactNode,props:P){
 hooks.cursor=0;
 const tree=nodes(Component(props));
 flushEffects();
 return tree;
}
async function settle(){for(let i=0;i<10;i++)await Promise.resolve();}
const nowSec=Math.floor(Date.now()/1000);
describe('AccessPanel LAN management and pairing codes',()=>{
 beforeEach(()=>{
  resetHooks();
  vi.clearAllMocks();
  mockApi.api.mockImplementation(async(path:string)=>{
   if(path==='/access/status')return {configured:true,transport:'loopback'};
   if(path==='/access/devices')return [{id:'dev-1',name:'My Laptop',role:'desktop',created:nowSec-60,expires:null,last_used:nowSec-10,revoked:false}];
   if(path==='/access/pairing-codes')return [{id:'code-1',name:'Pending Tablet',role:'web',expires:nowSec+240}];
   if(path==='/access/network')return {
    status:'configured',
    settings:{version:1,enabled:true,address:'192.168.50.154',port:14830,hostnames:['nas.local']},
    certificate_sha256:'AA:BB:CC',
    spki_fingerprint:'sha256:SPKI123',
   };
   return {};
  });
 });
 afterEach(()=>{resetHooks();vi.unstubAllGlobals();});

 it('loads and saves LAN network settings, displays fingerprints and restart notice',async()=>{
  render(AccessPanel,{} as never);
  await settle();
  let tree=render(AccessPanel,{} as never);
  const fullText=tree.map(textOf).join(' ');
  expect(fullText).toContain('AA:BB:CC');
  expect(fullText).toContain('sha256:SPKI123');
  const lanForm=tree.find(el=>el.type==='form'&&el.props['aria-label']==='lanAccessTitle')!;
  const labels=nodes(lanForm).filter(el=>el.type==='label');
  const addrInput=nodes(labels.find(l=>textOf(l).includes('lanAddress'))!).find(el=>el.type==='input')!;
  const portInput=nodes(labels.find(l=>textOf(l).includes('lanPort'))!).find(el=>el.type==='input')!;
  const hostInput=nodes(labels.find(l=>textOf(l).includes('lanHostnames'))!).find(el=>el.type==='input')!;
  expect(addrInput.props.value).toBe('192.168.50.154');
  expect(portInput.props.value).toBe('14830');
  expect(hostInput.props.value).toBe('nas.local');

  addrInput.props.onChange({target:{value:'192.168.50.200'}});
  portInput.props.onChange({target:{value:'14831'}});
  hostInput.props.onChange({target:{value:'nas.local, galroon.lan'}});
  mockApi.api.mockImplementation(async(path:string,body?:unknown)=>{
   if(path==='/access/network'&&body)return {settings:{version:1,...(body as object)},restart_required:true,message:'Restart Core now.'};
   if(path==='/access/status')return {configured:true};
   if(path==='/access/devices')return [];
   if(path==='/access/pairing-codes')return [];
   if(path==='/access/network')return {status:'configured',settings:{version:1,enabled:true,address:'192.168.50.200',port:14831,hostnames:['nas.local','galroon.lan']},certificate_sha256:'AA:BB:CC',spki_fingerprint:'sha256:SPKI123'};
   return {};
  });
  tree=render(AccessPanel,{} as never);
  tree.find(el=>el.type==='form'&&el.props['aria-label']==='lanAccessTitle')!.props.onSubmit({preventDefault:vi.fn()});
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/access/network',{enabled:true,address:'192.168.50.200',port:14831,hostnames:['nas.local','galroon.lan']});
  tree=render(AccessPanel,{} as never);
  expect(tree.some(el=>el.props.role==='status'&&textOf(el).includes('Restart Core now.'))).toBe(true);
 });

 it('creates pairing code with selected role, copies pairing string, and revokes pending codes',async()=>{
  const writeText=vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal('navigator',{clipboard:{writeText}});
  let pendingCodes=[{id:'code-1',name:'Pending Tablet',role:'web',expires:nowSec+240}];
  mockApi.api.mockImplementation(async(path:string,body?:unknown)=>{
   if(path==='/access/status')return {configured:true};
   if(path==='/access/devices')return [];
   if(path==='/access/pairing-codes')return pendingCodes;
   if(path==='/access/network')return {status:'configured',settings:{version:1,enabled:true,address:'192.168.50.154',port:14830,hostnames:[]}};
   if(path==='/access/pairing'){
    const created={id:'code-2',name:(body as any).name,role:(body as any).role,code:'ABCD-1234-EFGH',expires:nowSec+300,pairing_string:'galroon-pair:v1?host=192.168.50.154',certificate_sha256:'AA:BB:CC'};
    pendingCodes=[...pendingCodes,{id:'code-2',name:created.name,role:created.role,expires:created.expires}];
    return created;
   }
   if(path==='/access/pairing-codes/code-1/revoke'){
    pendingCodes=pendingCodes.filter(c=>c.id!=='code-1');
    return {revoked:true};
   }
   return {};
  });
  render(AccessPanel,{} as never);
  await settle();
  let tree=render(AccessPanel,{} as never);
  const nameLabel=tree.find(el=>el.type==='label'&&textOf(el).includes('deviceName'))!;
  nodes(nameLabel).find(el=>el.type==='input')!.props.onChange({target:{value:'Living Room Browser'}});
  const roleLabel=tree.find(el=>el.type==='label'&&textOf(el).includes('pairRole'))!;
  nodes(roleLabel).find(el=>el.type==='select')!.props.onChange({target:{value:'web'}});
  tree=render(AccessPanel,{} as never);
  tree.find(el=>el.type==='button'&&textOf(el)==='createPairCode')!.props.onClick();
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/access/pairing',{name:'Living Room Browser',role:'web'});

  tree=render(AccessPanel,{} as never);
  const pairBox=tree.find(el=>el.props.className==='pair-code')!;
  expect(textOf(pairBox)).toContain('ABCD-1234-EFGH');
  expect(textOf(pairBox)).toContain('galroon-pair:v1?host=192.168.50.154');
  nodes(pairBox).find(el=>el.type==='button'&&textOf(el).includes('copyPairingString'))!.props.onClick();
  await settle();
  expect(writeText).toHaveBeenCalledWith('galroon-pair:v1?host=192.168.50.154');

  // Revoke the first pending pairing code
  tree=render(AccessPanel,{} as never);
  const pendingRow=tree.find(el=>el.props.className==='access-device'&&textOf(el).includes('Pending Tablet'))!;
  nodes(pendingRow).find(el=>el.type==='button'&&textOf(el)==='revokePairingCode')!.props.onClick();
  await settle();
  expect(mockApi.api).toHaveBeenCalledWith('/access/pairing-codes/code-1/revoke',{});
  tree=render(AccessPanel,{} as never);
  expect(tree.some(el=>el.props.className==='access-device'&&textOf(el).includes('Pending Tablet'))).toBe(false);
 });
});

describe('Remote-Core folder picker fix (LAN R4)',()=>{
 beforeEach(()=>{resetHooks();vi.clearAllMocks();});
 afterEach(()=>{resetHooks();});

 it('hides Browse and shows Core path hint in AcquirePanel when not local desktop',()=>{
  mockApi.isLocalDesktop.mockReturnValue(false);
  const tree=render(AcquirePanel,{resource:{id:'r1',title:'Game',path:'/games/g',kind:'directory',work_id:null,release:'',files:1,bytes:100,root_id:'rt1',missing:0,unverified:0,revision:1,bindings:[]},onStarted:vi.fn(),deviceName:'NAS'});
  expect(tree.some(el=>el.type==='button'&&textOf(el).includes('browse'))).toBe(false);
  expect(tree.some(el=>textOf(el).includes('corePathHint'))).toBe(true);
  expect(mockApi.folder).not.toHaveBeenCalled();
 });

 it('hides Browse and shows Core path hint in SourceRemap when not local desktop',()=>{
  mockApi.isLocalDesktop.mockReturnValue(false);
  const tree=render(SourceRemap,{root:{id:'rt1',path:'/games',label:'Games',role:'source',online:true,missing:0,unverified:0},onClose:vi.fn(),onSaved:vi.fn()});
  expect(tree.some(el=>el.type==='button'&&textOf(el).includes('browse'))).toBe(false);
  expect(tree.some(el=>textOf(el).includes('corePathHint'))).toBe(true);
 });

 it('uses Core path text input instead of native folder() in DuplicatePanel when remote',async()=>{
  mockApi.isLocalDesktop.mockReturnValue(false);
  const onPlan=vi.fn();
  mockApi.api.mockImplementation(async(path:string)=>{
   if(path==='/duplicate-checks/j1')return {
    job_id:'j1',state:'completed',candidates:2,verified:2,stale:0,errors:[],
    groups:[{sha256:'abc',files:[{id:'f1',path:'/games/a.bin',size:10},{id:'f2',path:'/games/b.bin',size:10}]}],
   };
   if(path==='/quarantine')return {id:'p1',kind:'quarantine',state:'ready',digest:'d',items:[],operations:[]};
   return {};
  });
  render(DuplicatePanel,{roots:[{id:'rt1',path:'/games',label:'Games',role:'source',online:true,missing:0,unverified:0}],jobs:[{id:'j1',kind:'hash',state:'completed',processed:2,discovered:2,bytes:20,errors:0,message:'',created:1}],onStarted:vi.fn(),onPlan,onActivity:vi.fn()});
  await settle();
  let tree=render(DuplicatePanel,{roots:[{id:'rt1',path:'/games',label:'Games',role:'source',online:true,missing:0,unverified:0}],jobs:[{id:'j1',kind:'hash',state:'completed',processed:2,discovered:2,bytes:20,errors:0,message:'',created:1}],onStarted:vi.fn(),onPlan,onActivity:vi.fn()});
  const isoBtn=tree.find(el=>el.type==='button'&&textOf(el)==='isolate')!;
  expect(isoBtn.props.disabled).toBe(true);
  const qLabel=tree.find(el=>el.type==='label'&&textOf(el).includes('quarantineFolder'))!;
  nodes(qLabel).find(el=>el.type==='input')!.props.onChange({target:{value:'/data/quarantine'}});
  tree=render(DuplicatePanel,{roots:[{id:'rt1',path:'/games',label:'Games',role:'source',online:true,missing:0,unverified:0}],jobs:[{id:'j1',kind:'hash',state:'completed',processed:2,discovered:2,bytes:20,errors:0,message:'',created:1}],onStarted:vi.fn(),onPlan,onActivity:vi.fn()});
  const enabledIso=tree.find(el=>el.type==='button'&&textOf(el)==='isolate')!;
  expect(enabledIso.props.disabled).toBe(false);
  enabledIso.props.onClick();
  await settle();
  expect(mockApi.folder).not.toHaveBeenCalled();
  expect(mockApi.api).toHaveBeenCalledWith('/quarantine',{file_id:'f2',retained_id:'f1',destination:'/data/quarantine'});
  expect(onPlan).toHaveBeenCalledTimes(1);
 });
});
