import {useContext,useEffect,useState} from 'react';
import {readApi,type Work,type Resource,type Edition} from './api';
import {CatalogGenerationContext} from './useOwnedWorks';
export type WorkEditionSummary=Omit<Edition,'work_ids'>&{work_id:string;work_count:number};
export type WorkAssets={work:Work;resources:Resource[];editions:WorkEditionSummary[];revision:number;library_id:string};
export function useWorkAssets(work:Work){
 const generation=useContext(CatalogGenerationContext),[attempt,setAttempt]=useState(0),key=JSON.stringify([work.id,work.revision,generation,attempt]);
 const [state,setState]=useState<{key:string;data:WorkAssets|null;error:string}|null>(null);
 useEffect(()=>{const controller=new AbortController();void readApi<WorkAssets>(`/works/${encodeURIComponent(work.id)}/assets`,controller.signal).then(data=>{if(!controller.signal.aborted)setState({key,data,error:''});}).catch(e=>{if(!controller.signal.aborted)setState({key,data:null,error:e instanceof Error?e.message:String(e)});});return()=>controller.abort();},[key,work.id]);
 return {data:state?.key===key?state.data:null,error:state?.key===key?state.error:'',loading:state?.key!==key,reload:()=>setAttempt(n=>n+1)};
}
