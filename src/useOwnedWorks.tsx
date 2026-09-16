import {createContext,useContext,useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import type {Work} from './api';
import {createOwnedResolver} from './ownedWorks';
export const CatalogGenerationContext=createContext(0);
export function useOwnedWorks(ids:string[],active=true,local=false){
 const generation=useContext(CatalogGenerationContext),[attempt,setAttempt]=useState(0);
 const key=JSON.stringify([[...new Set(ids)].sort(),generation,attempt,local]);
 const [result,setResult]=useState<{key:string;works:Work[];error:string}|null>(null);
 useEffect(()=>{
  if(!active)return;const controller=new AbortController();let current=true;const requested=JSON.parse(key)[0] as string[];
  if(!requested.length){setResult({key,works:[],error:''});return;}
  const resolver=createOwnedResolver(controller.signal,local);
  void resolver.resolve(requested).then(works=>{if(current)setResult({key,works,error:''});}).catch(e=>{if(current)setResult({key,works:[],error:e instanceof Error?e.message:String(e)});});
  return()=>{current=false;controller.abort();};
 },[key,active]);
 const ready=result?.key===key;
 return {works:ready?result.works:[],loading:!ready&&ids.length>0,error:ready?result.error:'',retry:()=>setAttempt(n=>n+1)};
}
export function OwnedWorksNotice({state}:{state:ReturnType<typeof useOwnedWorks>}){
 const {t}=useTranslation();return state.loading?<p role="status">{t('checkingCollection')}</p>:state.error?<div className="profile-notice" role="alert"><p>{state.error}</p><button onClick={state.retry}>{t('retry')}</button></div>:null;
}
