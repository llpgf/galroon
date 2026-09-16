import {createOwnedResolver} from './ownedWorks';
import {useOwnedWorks,OwnedWorksNotice} from './useOwnedWorks';
import {useRelationshipRevision} from './relationshipChanges';
import {AddToList,type ListMember} from './AddToList';
import {completeProfileScope,type ManualProfilePage} from './manualProfile';
import {useDiscoveryPages} from './useDiscoveryPages';
import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ArrowRight,Search,RefreshCw} from 'lucide-react';
import {readApi} from './api';
import {Picture} from './ProfileVisuals';
import {discoveryPath,manualDiscoveryPath,mergeResults,type DiscoveryQuery,type DiscoveryData,type DiscoveryWork,type DiscoveryCharacter} from './discovery';
import type {Character} from './exploration';

export function DiscoverySearch({canWrite=false,visible,query,safe,spoilers,onSpoilers,onCharacter,onWork}:{canWrite?:boolean;visible:boolean;query:DiscoveryQuery;safe:boolean;spoilers:boolean;onSpoilers:()=>void;onCharacter:(character:Character)=>void;onWork:(work:DiscoveryWork)=>void}){
 const {t}=useTranslation();const heading=useRef<HTMLHeadingElement>(null);useEffect(()=>heading.current?.focus({preventScroll:true}),[]);
 const [target,setTarget]=useState<'works'|'characters'>(query.kind==='trait'?'characters':'works');
 return <section className="discovery-search">
  <header className="discovery-heading"><p className="eyebrow">{t('discoverySource')}</p><h1 ref={heading} tabIndex={-1}>{t('discoveryTitle')}</h1><div className="discovery-condition"><Search size={16}/><span>{t('discovery_'+query.kind)}</span><strong>{query.label}</strong></div></header>
  <div className="discovery-toolbar"><nav className="tabs" aria-label={t('discoveryResultType')}>{(query.kind==='trait'?['characters','works'] as const:['works'] as const).map(value=><button key={value} aria-pressed={target===value} className={target===value?'selected':''} onClick={()=>setTarget(value)}>{t('discovery_'+value)}</button>)}</nav>{query.kind==='trait'&&<button aria-pressed={spoilers} onClick={onSpoilers}>{t(spoilers?'showingSpoilers':'noSpoilers')}</button>}</div>
  {(query.kind==='trait'?['characters','works'] as const:['works'] as const).map(value=><div key={value} hidden={target!==value}>{/* Mount on first use; keep loaded pages while switching tabs. */}<Results canWrite={canWrite} key={String(spoilers)} active={visible&&target===value} query={query} target={value} safe={safe} spoilers={spoilers} onCharacter={onCharacter} onWork={onWork}/></div>)}
 </section>;
}
function Results({canWrite,active,query,target,safe,spoilers,onCharacter,onWork}:{canWrite:boolean;active:boolean;query:DiscoveryQuery;target:'works'|'characters';safe:boolean;spoilers:boolean;onCharacter:(character:Character)=>void;onWork:(work:DiscoveryWork)=>void}){
 const {t}=useTranslation();const relationRevision=useRelationshipRevision();
 const {scope,data,items,loading,manualLoading,error,manualError,incomplete,changed,manualMore,refresh,retry,retryManual,loadMore,loadMoreManual,cancelProvider,cancelManual}=useDiscoveryPages(query,target,spoilers,relationRevision,active);
 const ownership=useOwnedWorks(items.flatMap(item=>target==='works'?[(item as DiscoveryWork).id]:(item as DiscoveryCharacter).vns?.filter(v=>spoilers||v.spoiler===0).map(v=>v.id)||[]),active),works=ownership.works;
 const [listMembers,setListMembers]=useState<{scope:string;members:ListMember[]}|null>(null),[preparing,setPreparing]=useState(false),[count,setCount]=useState(0),[listError,setListError]=useState('');
 const preparation=useRef<{cancelled:boolean;controller:AbortController}|null>(null),currentScope=useRef(scope);currentScope.current=scope;
 useEffect(()=>{setPreparing(false);setListError('');return()=>{if(preparation.current){preparation.current.cancelled=true;preparation.current.controller.abort();preparation.current=null;}};},[active,scope]);
 async function prepareList(){
  if(target!=='works'||!canWrite||preparation.current)return;
  const operation={cancelled:false,controller:new AbortController()};preparation.current=operation;setPreparing(true);setCount(0);setListError('');
  try{
   const resolver=createOwnedResolver(operation.controller.signal);
   const members=await completeProfileScope(async(page,revision)=>{const result=await readApi<DiscoveryData>(discoveryPath(query,'works',page,spoilers,revision),operation.controller.signal);return {...result,works:result.results as DiscoveryWork[]};},(after,revision)=>readApi<ManualProfilePage>(manualDiscoveryPath(query,'works',spoilers,after,revision),operation.controller.signal),{kind:'search',character:{id:'',name:''},works:[],resolveOwned:resolver.resolve,ownedOnly:false,spoilers,cancelled:()=>operation.cancelled||currentScope.current!==scope,progress:setCount});await resolver.verify();
   if(!operation.cancelled&&currentScope.current===scope){if(members.length)setListMembers({scope,members});else setListError(t('listScopeEmpty'));}
  }catch(error){if(!operation.cancelled&&currentScope.current===scope)setListError(t(error instanceof Error?error.message:String(error)));}
  finally{if(preparation.current===operation){preparation.current=null;setPreparing(false);}}
 }
 return <>
  {canWrite&&target==='works'&&<div className="entity-actions"><button disabled={preparing||loading||manualLoading||incomplete||!!error||!!manualError||!!data?.stale} onClick={()=>void prepareList()}>{t('listAddSearchScope')}</button>{preparing&&<><span role="status">{t('listPreparingScope',{count})}</span><button onClick={()=>{if(preparation.current){preparation.current.cancelled=true;preparation.current.controller.abort();preparation.current=null;}setPreparing(false);}}>{t('cancel')}</button></>}</div>}
  {listError&&<p role="alert">{listError}</p>}
  {listMembers?.scope===scope&&active&&<AddToList members={listMembers.members} onClose={()=>setListMembers(null)}/>}
  <p className="discovery-count" role="status">{t(data?.more||manualMore?'discoveryLoadedMore':'discoveryLoaded',{count:items.length})}</p>
  {(error||data?.stale)&&<div role="alert" className="profile-notice"><p>{t(error)||data?.warning}</p><button disabled={loading} onClick={retry}>{t('retry')}</button></div>}
  <p>{t('discoveryManualOrder')}</p>
  <button disabled={loading||manualLoading} onClick={refresh}>{t('discoveryRefresh')}</button>
  {manualLoading&&<div><p role="status">{t('profileManualLoading')}</p><button onClick={cancelManual}>{t('profileCancelManual')}</button></div>}
  {manualError&&<div role="alert" className="profile-notice"><p>{t(manualError)}</p><button disabled={manualLoading} onClick={retryManual}>{t('profileManualRetry')}</button></div>}
  {incomplete&&<p role="alert">{t(changed?'discoveryRelationsChanged':'profileRelationsIncomplete')}</p>}
  <OwnedWorksNotice state={ownership}/>
  <div className={'discovery-grid discovery-'+target}>{!ownership.loading&&!ownership.error&&items.map(item=>target==='characters'?(()=>{const character=item as DiscoveryCharacter;const appearances=character.vns?.filter(v=>spoilers||v.spoiler===0)||[];const unique=mergeResults([],appearances);const owned=unique.some(v=>works.some(w=>w.vndb_id===v.id));return <article className="discovery-card" key={character.id}><button className="discovery-card-link" onClick={()=>onCharacter(character)}><Picture name={character.name} image={character.image} hidden={safe}/><strong>{character.name}</strong>{character.original&&character.original!==character.name&&<small>{character.original}</small>}</button>{owned&&<span className="discovery-owned">{t('inCollection')}</span>}<div className="discovery-appearances">{unique.slice(0,2).map(v=><button className="metadata-link" key={v.id} onClick={()=>onWork({id:v.id,title:v.title||v.id})}>{v.title||v.id}</button>)}{unique.length>2&&<small>{t('discoveryOtherWorks',{count:unique.length-2})}</small>}</div></article>;})():(()=>{const work=item as DiscoveryWork;const owned=works.some(w=>w.vndb_id===work.id);return <article className="discovery-card" key={work.id}><button className="discovery-card-link" onClick={()=>onWork(work)}><Picture name={work.title} image={work.image} hidden={safe}/><strong>{work.title}</strong>{work.alttitle&&work.alttitle!==work.title&&<small>{work.alttitle}</small>}</button><p>{work.developers?.map(d=>d.name).join(', ')}</p><p>{work.released}</p>{owned&&<span className="discovery-owned">{t('inCollection')}</span>}</article>;})())}</div>
  {loading&&<div><p className="explore-status" role="status"><RefreshCw className="spin" size={16}/>{t('discoveryLoading')}</p><button onClick={cancelProvider}>{t('discoveryCancel')}</button></div>}
  {!loading&&!manualLoading&&!error&&!manualError&&!incomplete&&data&&!items.length&&<p className="empty">{t(target==='characters'?'noCharacters':'discoveryEmpty')}</p>}
  {data?.more&&!data.partial&&!error&&<button className="profile-load-more" disabled={loading} onClick={loadMore}>{t('discoveryLoadMore')}<ArrowRight size={16}/></button>}
  {manualMore&&!manualError&&<button disabled={manualLoading} className="profile-load-more" onClick={loadMoreManual}>{t('profileManualMore')}<ArrowRight size={16}/></button>}
 </>;
}
