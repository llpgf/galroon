import {useEffect,useRef,useState} from 'react';
import {isDesktop} from './api';
import {APP_VERSION} from './version';
import {applyCoreUpdate,applyDesktopUpdate,checkUpdates,compareSemver,readUpdates,setUpdateSettings,waitForCoreRestart,type UpdateState} from './updates';
export type UpdateProgress={
 phase:'idle'|'core_downloading'|'core_verifying'|'core_staging'|'core_restarting'|'desktop_downloading'|'desktop_installing'|'reloading';
 targetVersion?:string;
 downloaded?:number;
 total?:number|null;
};
const IDLE:UpdateProgress={phase:'idle'};
export function useUpdates(core:string|null,coreVersion?:string){
 const [state,setState]=useState<UpdateState|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 const [progress,setProgress]=useState<UpdateProgress>(IDLE),[needsForceConfirm,setNeedsForceConfirm]=useState(false);
 const epoch=useRef(0),revision=useRef(0),applyingRef=useRef(false);
 useEffect(()=>{const current=++epoch.current;setState(null);setError('');setBusy(false);if(!applyingRef.current)setProgress(IDLE);setNeedsForceConfirm(false);if(!core)return;
  const read=async()=>{const version=revision.current;try{const value=await readUpdates();if(epoch.current===current&&revision.current===version){setState(value);setError('');}}catch(e){if(epoch.current===current&&revision.current===version)setError(e instanceof Error?e.message:String(e));}};
  void read();const timer=setInterval(()=>void read(),6*60*60*1000);return()=>{epoch.current++;clearInterval(timer);};
 },[core]);
 const run=async(action:()=>Promise<UpdateState>)=>{const current=epoch.current;revision.current++;setBusy(true);setError('');try{const value=await action();if(epoch.current===current)setState(value);}catch(e){if(epoch.current===current)setError(e instanceof Error?e.message:String(e));}finally{if(epoch.current===current)setBusy(false);}};
 const targetVersion=state?.latest?.version||(coreVersion&&compareSemver(coreVersion,APP_VERSION)>0?coreVersion:undefined);
 const canUpdateCore=!!state?.can_self_update&&!!state.update_available&&!!state.latest;
 const canUpdateDesktop=isDesktop&&!!targetVersion&&compareSemver(targetVersion,APP_VERSION)>0;
 const canApply=canUpdateCore||canUpdateDesktop;
 const apply=async(force=false)=>{
  if(applyingRef.current||!canApply)return;
  applyingRef.current=true;setError('');setNeedsForceConfirm(false);
  const previousCore=state?.current||coreVersion||APP_VERSION;
  const target=targetVersion||state?.latest?.version||'';
  try{
   if(canUpdateCore&&state?.latest){
    setProgress({phase:'core_downloading',targetVersion:state.latest.version,downloaded:0,total:null});
    let latestState=await applyCoreUpdate(force);
    setState(latestState);
    while(latestState.apply&&['downloading','verifying','staging'].includes(latestState.apply.state)){
     const s=latestState.apply;
     setProgress({
      phase:s.state==='downloading'?'core_downloading':s.state==='verifying'?'core_verifying':'core_staging',
      targetVersion:s.target_version||state.latest.version,
      downloaded:s.downloaded_bytes,
      total:s.total_bytes
     });
     await new Promise(r=>setTimeout(r,600));
     try{latestState=await readUpdates();setState(latestState);}catch{break;}
    }
    if(latestState.apply?.state==='failed'){
     throw new Error(latestState.apply.error||'Core update failed.');
    }
    setProgress({phase:'core_restarting',targetVersion:state.latest.version});
    const restart=await waitForCoreRestart(previousCore,state.latest.version);
    setState(restart.state);
    if(restart.status==='rolled_back'){throw new Error(restart.error);}
    if(!isDesktop){
     setProgress({phase:'reloading',targetVersion:state.latest.version});
     window.location.reload();
     return;
    }
   }
   if(canUpdateDesktop){
    setProgress({phase:'desktop_downloading',targetVersion:target,downloaded:0,total:null});
    await applyDesktopUpdate(({stage,downloaded,total})=>{
     setProgress({phase:stage==='downloading'?'desktop_downloading':'desktop_installing',targetVersion:target,downloaded,total});
    });
   }
  }catch(e){
   const message=e instanceof Error?e.message:String(e);
   setError(message);
   if(message.includes('Active background tasks'))setNeedsForceConfirm(true);
   setProgress(IDLE);
  }finally{
   applyingRef.current=false;
  }
 };
 return {state,error,busy,applying:progress.phase!=='idle',progress,canApply,canUpdateCore,canUpdateDesktop,needsForceConfirm,check:()=>run(checkUpdates),setEnabled:(enabled:boolean)=>run(()=>setUpdateSettings(enabled)),apply};
}
