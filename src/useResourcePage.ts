import {useEffect,useMemo,useState} from 'react';
import {readApi,type Resource} from './api';
export type ResourcePage={items:Resource[];next:string|null;total:number;offset:number;revision:number;library_id:string};

/** Never display a previous query's rows as the result of the current query. */
export function useResourcePage(query:string,status:string,generation:number,rootId='',excludeId=''){
 const serialized=JSON.stringify([query,status,rootId,excludeId]),[retry,setRetry]=useState(0);
 const key=serialized+'\n'+generation+'\n'+retry;
 const [trail,setTrail]=useState<{key:string;cursors:(string|null)[]}>({key:'',cursors:[null]});
 const cursors=useMemo(()=>trail.key===key?trail.cursors:[null],[trail,key]);
 const before=cursors.at(-1)||null;
 const [state,setState]=useState<{key:string;before:string|null;data:ResourcePage|null;error:string}>({key:'',before:null,data:null,error:''});
 useEffect(()=>{
  const controller=new AbortController();
  const params=new URLSearchParams({query,status,root_id:rootId,exclude_id:excludeId});if(before)params.set('before',before);
  const timer=setTimeout(()=>{void readApi<ResourcePage>('/resources/page?'+params,controller.signal).then(data=>{if(!controller.signal.aborted)setState({key,before,data,error:''});}).catch(error=>{if(!controller.signal.aborted)setState({key,before,data:null,error:error instanceof Error?error.message:String(error)});});},before?0:200);
  return()=>{clearTimeout(timer);controller.abort();};
 },[key,before,serialized]);
 const current=state.key===key&&state.before===before;
 const data=current?state.data:null,error=current?state.error:'';
 return {data,error,loading:!current,metadata:state.data,reload:()=>setRetry(n=>n+1),
  next:()=>{if(data?.next)setTrail({key,cursors:[...cursors,data.next]});},
  previous:()=>{if(cursors.length>1)setTrail({key,cursors:cursors.slice(0,-1)});},
  pageIndex:cursors.length-1};
}
