import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {readApi} from './api';
import {emptyFilters} from './library';
import {useCollectionPage} from './useCollectionPage';
export type MembershipDelta=Record<string,{title:string;selected:boolean}>;
export type EditionMembershipBase={id:string;revision:number;collection_revision:number;library_id:string;work_count:number};
type MembershipPage={ids:string[];members:string[];revision:number;collection_revision:number;library_id:string};
export function validateMembershipPage(data:MembershipPage,ids:string[],base:EditionMembershipBase){
 const requested=new Set(ids),members=new Set(data.members);
 if(data.revision!==base.revision||data.collection_revision!==base.collection_revision||data.library_id!==base.library_id||data.ids.length!==ids.length||new Set(data.ids).size!==ids.length||data.ids.some(id=>!requested.has(id))||members.size!==data.members.length||data.members.some(id=>!requested.has(id)))throw new Error('Edition membership could not be verified. Close and reopen the editor.');
 return members;
}
export function membershipCount(base:EditionMembershipBase|null,delta:MembershipDelta){return (base?.work_count||0)+Object.values(delta).reduce((n,item)=>n+(item.selected?1:-1),0);}
export function EditionWorkPicker({base,delta,onChange,onReady,disabled}:{base:EditionMembershipBase|null;delta:MembershipDelta;onChange:(next:MembershipDelta)=>void;onReady:(ready:boolean)=>void;disabled:boolean}){
 const {t,i18n}=useTranslation(),[query,setQuery]=useState('');
 const page=useCollectionPage({filters:emptyFilters,query,status:'all',sort:'title',titleMode:'display',locale:i18n.language},0);
 const ids=JSON.stringify(page.data?.items.map(w=>w.id)||[]),key=JSON.stringify([base,ids,page.data?.revision,page.data?.library_id]);
 const [state,setState]=useState<{key:string;members:Set<string>;error:string}|null>(null),[attempt,setAttempt]=useState(0);
 useEffect(()=>{if(!page.data)return;const controller=new AbortController();
  if(!base){setState({key,members:new Set(),error:''});return;}
  const params=new URLSearchParams({ids,revision:String(base.revision),collection_revision:String(base.collection_revision),library_id:base.library_id});
  void readApi<MembershipPage>(`/releases/${encodeURIComponent(base.id)}/membership?${params}`,controller.signal).then(data=>{const members=validateMembershipPage(data,JSON.parse(ids),base);if(!controller.signal.aborted)setState({key,members,error:''});}).catch(e=>{if(!controller.signal.aborted)setState({key,members:new Set(),error:e instanceof Error?e.message:String(e)});});return()=>controller.abort();
 },[key,attempt]);
 const current=state?.key===key,ready=!page.loading&&!page.error&&current&&!state.error,error=page.error||(current?state.error:'');
 useEffect(()=>onReady(!!ready),[ready,onReady]);
 return <div className="edition-work-picker"><p>{t('editionSelectedCount',{count:membershipCount(base,delta)})}</p><p>{t('editionOffPagePreserved')}</p>
 <label>{t('editionFindWorks')}<input type="search" value={query} disabled={disabled} onChange={e=>setQuery(e.target.value)}/></label>
 {(!ready&&!error)&&<p role="status">{t('loading')}</p>}{error&&<div role="alert"><p>{error}</p><button disabled={disabled} onClick={()=>{setAttempt(n=>n+1);page.reload();}}>{t('retry')}</button></div>}
 {ready&&page.data?.items.map(w=>{const was=state.members.has(w.id),selected=delta[w.id]?.selected??was;return <label className="checkbox-label" key={w.id}><input disabled={disabled} type="checkbox" checked={selected} onChange={e=>{const next={...delta};if(e.target.checked===was)delete next[w.id];else next[w.id]={title:w.title,selected:e.target.checked};onChange(next);}}/>{w.title}</label>;})}
 {ready&&page.data?.items.length===0&&<p>{t('noResults')}</p>}
 <div className="inline"><button disabled={disabled||page.loading||page.pageIndex===0} onClick={page.previous}>{t('editionPickerPrevious')}</button><span>{page.pageIndex+1}</span><button disabled={disabled||!ready||!page.data?.next} onClick={page.next}>{t('editionPickerNext')}</button></div>
 {Object.keys(delta).length>0&&<section aria-label={t('editionMembershipChanges')}><h4>{t('editionMembershipChanges')}</h4>{Object.entries(delta).map(([id,value])=><div key={id}><span>{t(value.selected?'editionMemberAdded':'editionMemberRemoved')}: {value.title}</span><button disabled={disabled} onClick={()=>{const next={...delta};delete next[id];onChange(next);}}>{t('editionUndoChange')}</button></div>)}</section>}
 </div>;
}
