import {useEffect,useState} from 'react';
import {readApi} from './api';import type {Exploration} from './exploration';
/** An old spoiler scope must never remain in the DOM during the next request. */
export function useWorkExploration(path:string,spoilers:boolean,revision:number,active=true){
 const scope=JSON.stringify([path,spoilers,revision]);
 const [response,setResponse]=useState<{scope:string;value:Exploration}|null>(null);
 const [state,setState]=useState({scope,loading:true,error:''}),[request,setRequest]=useState({scope,attempt:0});
 const refresh=request.scope===scope&&request.attempt>0;
 useEffect(()=>{if(!active)return;const controller=new AbortController();setState({scope,loading:true,error:''});
  void readApi<Exploration>(`${path}?spoilers=${spoilers}${refresh?'&refresh=true':''}`,controller.signal).then(value=>{if(!controller.signal.aborted)setResponse({scope,value});}).catch(e=>{if(!controller.signal.aborted)setState({scope,loading:false,error:e instanceof Error?e.message:String(e)});}).finally(()=>{if(!controller.signal.aborted)setState(previous=>({...previous,loading:false}));});return()=>controller.abort();
 },[scope,request,active]);
 return {data:response?.scope===scope?response.value:null,loading:active&&(state.scope!==scope||state.loading),error:state.scope===scope?state.error:'',refresh:()=>setRequest(previous=>({scope,attempt:previous.scope===scope?previous.attempt+1:1})),attempt:request.scope===scope?request.attempt:0};
}
