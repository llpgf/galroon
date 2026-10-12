import {useEffect,useRef,useState} from 'react';
import {readUpdates,checkUpdates,setUpdateSettings,type UpdateState} from './updates';
export function useUpdates(core:string|null){
 const [state,setState]=useState<UpdateState|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);const epoch=useRef(0),revision=useRef(0);
 useEffect(()=>{const current=++epoch.current;setState(null);setError('');setBusy(false);if(!core)return;
  const read=async()=>{const version=revision.current;try{const value=await readUpdates();if(epoch.current===current&&revision.current===version){setState(value);setError('');}}catch(e){if(epoch.current===current&&revision.current===version)setError(e instanceof Error?e.message:String(e));}};
  void read();const timer=setInterval(()=>void read(),6*60*60*1000);return()=>{epoch.current++;clearInterval(timer);};
 },[core]);
 const run=async(action:()=>Promise<UpdateState>)=>{const current=epoch.current;revision.current++;setBusy(true);setError('');try{const value=await action();if(epoch.current===current)setState(value);}catch(e){if(epoch.current===current)setError(e instanceof Error?e.message:String(e));}finally{if(epoch.current===current)setBusy(false);}};
 return {state,error,busy,check:()=>run(checkUpdates),setEnabled:(enabled:boolean)=>run(()=>setUpdateSettings(enabled))};
}
