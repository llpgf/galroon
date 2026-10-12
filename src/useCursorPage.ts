import {useEffect,useMemo,useState} from 'react';
import {readApi} from './api';

/** Core serves at most two collection reads at once and rejects the rest; such a refusal is worth retrying. */
export const isBusy=(error:unknown)=>error instanceof Error&&/queries are running\. Retry in a moment/.test(error.message);

/** Retries Core's busy refusals with growing delays; any other failure, or cancellation, ends at once. */
export async function retryWhileBusy<T>(read:()=>Promise<T>,signal:AbortSignal,delays=[150,300,600,1200,2400]):Promise<T>{
 for(let attempt=0;;attempt++){
  try{return await read();}catch(error){
   if(signal.aborted)throw new DOMException('Request cancelled','AbortError');
   if(!isBusy(error)||attempt>=delays.length)throw error;
   await new Promise<void>((resolve,reject)=>{const timer=setTimeout(resolve,delays[attempt]+Math.random()*100);signal.addEventListener('abort',()=>{clearTimeout(timer);reject(new DOMException('Request cancelled','AbortError'));},{once:true});});
  }
 }
}

/** A refresh replaces the current window atomically; only a new query or rejected cursor restarts it. */
export function useCursorPage<T extends {next:string|null}>(path:string,generation:number,staleMessage:string){
 const [retry,setRetry]=useState(0);
 const [trail,setTrail]=useState<{key:string;cursors:(string|null)[];restarted:boolean}>({key:'',cursors:[null],restarted:false});
 useEffect(()=>{setTrail(old=>old.key===path?old:{key:path,cursors:[null],restarted:false});},[path]);
 const cursors=useMemo(()=>trail.key===path?trail.cursors:[null],[trail,path]);
 const before=cursors.at(-1)||null;
 const [state,setState]=useState<{key:string;before:string|null;data:T|null;error:string}>({key:'',before:null,data:null,error:''});
 useEffect(()=>{
  const controller=new AbortController();
  const timer=setTimeout(()=>{void retryWhileBusy(()=>readApi<T>(path+(before?'&before='+encodeURIComponent(before):''),controller.signal),controller.signal).then(data=>{if(!controller.signal.aborted)setState({key:path,before,data,error:''});}).catch(error=>{
   if(controller.signal.aborted)return;
   const message=error instanceof Error?error.message:String(error);
   if(before&&message.startsWith(staleMessage)){setTrail({key:path,cursors:[null],restarted:true});return;}
   // A failed background read must not unmount the last successful window either.
   setState(old=>({key:path,before,data:old.key===path&&old.before===before?old.data:null,error:message}));
  });},before?0:200);
  return()=>{clearTimeout(timer);controller.abort();};
 },[path,before,generation,retry,staleMessage]);
 const current=state.key===path&&state.before===before;
 const data=current?state.data:null,error=current?state.error:'';
 return {data,error,loading:!current,metadata:state.data,restarted:trail.key===path&&trail.restarted,reload:()=>setRetry(n=>n+1),
  next:()=>{if(data?.next)setTrail({key:path,cursors:[...cursors,data.next],restarted:false});},
  previous:()=>{if(cursors.length>1)setTrail({key:path,cursors:cursors.slice(0,-1),restarted:false});},
  pageIndex:cursors.length-1};
}
