import {invoke} from '@tauri-apps/api/core';
import {api,connect,coreHealth,isDesktop} from './api';
export type UpdateRelease={version:string;name:string;url:string;notes:string;published_at:string|null};
export type UpdateApplyStatus={state:'idle'|'downloading'|'verifying'|'staging'|'restarting'|'failed';target_version:string|null;downloaded_bytes:number;total_bytes:number|null;error:string|null};
export type UpdateState={current:string;install_kind:'docker'|'desktop'|'standalone';can_self_update?:boolean;enabled:boolean;checking:boolean;last_checked:number|null;last_error:string|null;update_available:boolean;latest:UpdateRelease|null;apply?:UpdateApplyStatus};
export const readUpdates=()=>api<UpdateState>('/updates');
export const checkUpdates=()=>api<UpdateState>('/updates/check',{});
export const setUpdateSettings=(enabled:boolean)=>api<UpdateState>('/updates/settings',{enabled});
export const applyCoreUpdate=(force=false)=>api<UpdateState>('/updates/apply',force?{force:true}:{});
const SKIPPED='galroon.updates.skipped';
export function skippedVersion():string|null{try{return localStorage.getItem(SKIPPED);}catch{return null;}}
export function skipVersion(version:string):void{try{localStorage.setItem(SKIPPED,version);}catch{/* This client may have storage disabled. */}}
export function shouldShowUpdate(state:UpdateState|null,skipped:string|null):boolean{return !!state?.update_available&&!!state.latest&&state.latest.version!==skipped;}
export function compareSemver(a:string,b:string):number{
 const parse=(v:string)=>v.replace(/^v/,'').split('-')[0].split('.').map(n=>Number.parseInt(n,10)||0);
 const pa=parse(a),pb=parse(b);
 for(let i=0;i<3;i++){const d=(pa[i]||0)-(pb[i]||0);if(d!==0)return d>0?1:-1;}
 return 0;
}
export async function applyDesktopUpdate(onProgress:(info:{stage:'downloading'|'installing';downloaded:number;total:number|null})=>void):Promise<void>{
 if(!isDesktop)throw new Error('Desktop app required');
 const [{check},{relaunch}]=await Promise.all([import('@tauri-apps/plugin-updater'),import('@tauri-apps/plugin-process')]);
 const update=await check();
 if(!update)throw new Error('No newer signed desktop update is available yet.');
 let downloaded=0;let total:number|null=null;
 onProgress({stage:'downloading',downloaded:0,total:null});
 await update.download(event=>{
  if(event.event==='Started'){total=event.data.contentLength??null;onProgress({stage:'downloading',downloaded,total});}
  else if(event.event==='Progress'){downloaded+=event.data.chunkLength;onProgress({stage:'downloading',downloaded,total});}
  else if(event.event==='Finished'){onProgress({stage:'downloading',downloaded:total??downloaded,total});}
 });
 onProgress({stage:'installing',downloaded:total??downloaded,total});
 await invoke('prepare_desktop_update');
 await update.install();
 await relaunch();
}
export async function waitForCoreRestart(previousVersion:string,targetVersion:string,timeoutMs=90_000):Promise<{status:'updated';state:UpdateState}|{status:'rolled_back';error:string;state:UpdateState}>{
 const deadline=Date.now()+timeoutMs;
 let sawDisconnect=false;
 while(Date.now()<deadline){
  await new Promise(r=>setTimeout(r,1000));
  try{
   if(isDesktop){
    await connect().catch(()=>false);
   }else{
    const healthy=await coreHealth();
    if(!healthy){sawDisconnect=true;continue;}
   }
   const next=await readUpdates();
   if(compareSemver(next.current,targetVersion)>=0||compareSemver(next.current,previousVersion)>0){
    return {status:'updated',state:next};
   }
   if(next.apply?.state==='failed'){
    return {status:'rolled_back',error:next.apply.error||'Core update rolled back.',state:next};
   }
   if(sawDisconnect&&next.apply?.state==='idle'){
    return {status:'rolled_back',error:'Core restarted on the previous version.',state:next};
   }
  }catch{
   sawDisconnect=true;
  }
 }
 throw new Error('Timed out waiting for Core to restart after updating.');
}
