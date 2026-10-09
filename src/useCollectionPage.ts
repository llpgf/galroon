import {useEffect,useMemo,useState} from 'react';
import {readApi,type Work} from './api';
import type {Filters} from './library';

export type CollectionQuery={filters:Filters;query:string;status:string;sort:string;titleMode:'display'|'original';locale:string};
export type CollectionPage={items:Work[];next:string|null;total:number;filtered:number;offset:number;revision:number;library_id:string;view_token:string;options:Record<'studio'|'year'|'language'|'tag',{value:string;label:string}[]>;tag_counts:Record<string,number>;status_counts:Record<string,number>;custom_tags:{id:string;name:string}[]};

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

/** Never display a previous query's rows as the result of the current query. */
export function useCollectionPage(query:CollectionQuery,generation:number){
 const serialized=JSON.stringify(query),[retry,setRetry]=useState(0);
 const key=serialized+'\n'+generation+'\n'+retry;
 const [trail,setTrail]=useState<{key:string;cursors:(string|null)[]}>({key:'',cursors:[null]});
 const cursors=useMemo(()=>trail.key===key?trail.cursors:[null],[trail,key]);
 const before=cursors.at(-1)||null;
 const [state,setState]=useState<{key:string;before:string|null;data:CollectionPage|null;error:string}>({key:'',before:null,data:null,error:''});
 useEffect(()=>{
  const controller=new AbortController();
  const params=new URLSearchParams({query:serialized});if(before)params.set('before',before);
  const timer=setTimeout(()=>{void retryWhileBusy(()=>readApi<CollectionPage>('/collection?'+params,controller.signal),controller.signal).then(data=>{if(!controller.signal.aborted)setState({key,before,data,error:''});}).catch(error=>{if(!controller.signal.aborted)setState({key,before,data:null,error:error instanceof Error?error.message:String(error)});});},before?0:200);
  return()=>{clearTimeout(timer);controller.abort();};
 },[key,before,serialized]);
 const current=state.key===key&&state.before===before;
 const data=current?state.data:null,error=current?state.error:'';
 return {data,error,loading:!current,metadata:state.data,reload:()=>setRetry(n=>n+1),
  next:()=>{if(data?.next)setTrail({key,cursors:[...cursors,data.next]});},
  previous:()=>{if(cursors.length>1)setTrail({key,cursors:cursors.slice(0,-1)});},
  pageIndex:cursors.length-1};
}
