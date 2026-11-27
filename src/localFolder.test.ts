import {afterEach,beforeEach,describe,expect,it,vi} from 'vitest';
import type {Resource,Root} from './api';

const apiState=vi.hoisted(()=>({isDesktop:true,isLocalDesktop:true}));
vi.mock('./api',()=>({
 get isDesktop(){return apiState.isDesktop;},
 isLocalDesktop:()=>apiState.isLocalDesktop,
 localFolder:vi.fn(),
 revealInFolder:vi.fn(),
}));

import {joinResourcePath,primaryResourceForWork,resolveResourcePath,saveLocalRoot,savedLocalRoot} from './localFolder';

const roots:Root[]=[
 {id:'nas',path:'/volume1/Backup/Galgame',label:'NAS',role:'source',online:true,missing:0,unverified:0},
 {id:'offline',path:'D:\\Games',label:'Offline',role:'source',online:false,missing:0,unverified:0},
];
const res=(id:string,patch:Partial<Resource>={}):Resource=>({
 id,title:id,path:'[261127] Game',kind:'folder',work_id:'w1',release:'Unclassified',files:3,bytes:100,
 root_id:'nas',root_path:'/volume1/Backup/Galgame',missing:0,unverified:0,revision:1,
 bindings:[{work_id:'w1',release_id:null,role:'main'}],...patch,
});

let stored:Record<string,string>={};
beforeEach(()=>{
 stored={};
 apiState.isDesktop=true;apiState.isLocalDesktop=true;
 vi.stubGlobal('localStorage',{
  getItem:(k:string)=>stored[k]??null,
  setItem:(k:string,v:string)=>{stored[k]=v;},
  removeItem:(k:string)=>{delete stored[k];},
 });
});
afterEach(()=>vi.unstubAllGlobals());

describe('localFolder path resolution',()=>{
 it('joins POSIX, Windows drive, and UNC paths while ignoring dot traversal',()=>{
  expect(joinResourcePath('/volume1/Backup/Galgame','2026/[261127] Game')).toBe('/volume1/Backup/Galgame/2026/[261127] Game');
  expect(joinResourcePath('Z:\\Galgame\\','2026/[261127] Game')).toBe('Z:\\Galgame\\2026\\[261127] Game');
  expect(joinResourcePath('\\\\192.168.50.154\\Backup\\Galgame','../secret/[261127] Game')).toBe('\\\\192.168.50.154\\Backup\\Galgame\\secret\\[261127] Game');
 });

 it('opens local desktop paths directly and maps remote NAS roots via saved local folder',()=>{
  const r=res('r1');
  expect(resolveResourcePath(r,roots)).toMatchObject({openPath:'/volume1/Backup/Galgame/[261127] Game',needsMapping:false});

  apiState.isLocalDesktop=false;
  expect(resolveResourcePath(r,roots)).toMatchObject({openPath:null,needsMapping:true,corePath:'/volume1/Backup/Galgame/[261127] Game'});

  saveLocalRoot('nas','Z:\\Galgame');
  expect(savedLocalRoot('nas')).toBe('Z:\\Galgame');
  expect(resolveResourcePath(r,roots)).toMatchObject({openPath:'Z:\\Galgame\\[261127] Game',needsMapping:false,mappedRoot:'Z:\\Galgame'});

  saveLocalRoot('nas','');
  expect(savedLocalRoot('nas')).toBe('');
  expect(resolveResourcePath(r,roots).needsMapping).toBe(true);
 });

 it('prioritizes available preferred-edition main folder resource for Hero quick open',()=>{
  const extra=res('extra',{kind:'archive',bindings:[{work_id:'w1',release_id:'e1',role:'extra'}]});
  const offline=res('off',{root_id:'offline',bindings:[{work_id:'w1',release_id:'e1',role:'main'}]});
  const prefMain=res('pref',{kind:'folder',bindings:[{work_id:'w1',release_id:'e1',role:'main'}]});
  const otherMain=res('other',{kind:'folder',bindings:[{work_id:'w1',release_id:'e2',role:'main'}]});
  expect(primaryResourceForWork('w1','e1',[extra,offline,otherMain,prefMain],roots)?.id).toBe('pref');
 });
});
