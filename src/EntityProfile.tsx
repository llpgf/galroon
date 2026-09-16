import {createOwnedResolver} from './ownedWorks';
import {useOwnedWorks,OwnedWorksNotice} from './useOwnedWorks';
import {EntityFieldEditor} from './EntityFieldEditor';
import {useRelationshipRevision} from './relationshipChanges';
import {EntityTags} from './EntityTags';
import {AddToList,type ListMember} from './AddToList';
import {completeProfileScope,manualProfilePath,type ManualProfilePage} from './manualProfile';
import {useProfilePages} from './useProfilePages';
import {searchableDate,type DiscoveryQuery} from './discovery';
import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ArrowRight,ExternalLink,RefreshCw} from 'lucide-react';
import {readApi,type Work} from './api';
import {titleFor} from './library';
import {Avatar,Picture} from './ProfileVisuals';
import {plainDescription,safeExternalUrl,visibleCharacter,visibleTraits,type Person,type Character,type Company,type ProfileData} from './exploration';

export function EntityProfile({active=true,canWrite=false,kind,initial,safe,spoilers,onSpoilers,onWork,onPerson,onCharacter,onSearch,titleMode}:{active?:boolean;canWrite?:boolean;kind:'character'|'person'|'company';initial:Person|Character|Company;safe:boolean;spoilers:boolean;onSpoilers:()=>void;onWork:(work:Work)=>void;onPerson:(person:Person)=>void;onCharacter:(character:Character)=>void;onSearch:(query:DiscoveryQuery)=>void;titleMode:'display'|'original'}){
 const heading=useRef<HTMLHeadingElement>(null);useEffect(()=>heading.current?.focus({preventScroll:true}),[]);
 const {t,i18n}=useTranslation();const [scope,setScope]=useState('all'),[traitsExpanded,setTraitsExpanded]=useState(false);
 const [editingFields,setEditingFields]=useState(false);
 const relationRevision=useRelationshipRevision();
 const {data,credits,loading,manualLoading,error,manualError,incomplete,changed,manualMore,refresh,retry,retryManual,loadMore,loadMoreManual,cancelProvider,cancelManual}=useProfilePages(kind,initial.id,spoilers,relationRevision,active);
 const entity=(kind==='person'?data?.person:kind==='character'?data?.character:data?.company)||initial;
 const character=entity as Character,person=entity as Person,company=entity as Company;
 const eligible=credits.filter(work=>kind!=='character'||work.profile_membership===true||visibleCharacter(character,work.id,spoilers));
 const ownership=useOwnedWorks(eligible.map(work=>work.id),active),works=ownership.works;
 const visible=ownership.loading||ownership.error?[]:eligible.filter(work=>scope==='all'||works.some(owned=>owned.vndb_id===work.id));
 const listScope=JSON.stringify([kind,initial.id,spoilers,relationRevision,scope,active]);
 const [listMembers,setListMembers]=useState<{scope:string;members:ListMember[]}|null>(null),[preparing,setPreparing]=useState(false),[preparedCount,setPreparedCount]=useState(0),[listError,setListError]=useState('');
 const preparation=useRef<{cancelled:boolean;controller:AbortController}|null>(null);
 useEffect(()=>{setListMembers(null);return()=>{if(preparation.current){preparation.current.cancelled=true;preparation.current.controller.abort();preparation.current=null;}setPreparing(false);};},[kind,initial.id,active,spoilers,relationRevision,scope]);
 async function prepareList(){
  if(preparation.current&&!preparation.current.cancelled)return;
  const operation={cancelled:false,controller:new AbortController()};preparation.current=operation;setPreparing(true);setPreparedCount(0);setListError('');
  const endpoint={person:'people',character:'characters',company:'companies'}[kind];
  try{const resolver=createOwnedResolver(operation.controller.signal);const members=await completeProfileScope((page,revision)=>readApi<ProfileData>(`/${endpoint}/${initial.id}?page=${page}&spoilers=${spoilers}&revision=${revision}`,operation.controller.signal),(after,revision)=>readApi<ManualProfilePage>(manualProfilePath(initial.id,spoilers,after,revision),operation.controller.signal),{kind,character,works:[],resolveOwned:resolver.resolve,ownedOnly:scope==='owned',spoilers,cancelled:()=>operation.cancelled,progress:setPreparedCount});await resolver.verify();if(!operation.cancelled){if(members.length)setListMembers({scope:listScope,members});else setListError(t('listScopeEmpty'));}}
  catch(error){if(!operation.cancelled)setListError(t(error instanceof Error?error.message:String(error)));}
  finally{if(preparation.current===operation){preparation.current=null;setPreparing(false);}}
 }
 const aliases=[...new Set((entity.aliases||[]).flatMap(alias=>typeof alias==='string'?[alias]:[alias.name,alias.latin||'']).filter(name=>name&&name!==entity.name&&name!==entity.original))];
 const links=('extlinks' in entity?entity.extlinks||[]:[]).filter(link=>safeExternalUrl(link.url));
 const traits=kind==='character'?visibleTraits(character,spoilers,safe):[];
 const biography=plainDescription(entity.description||'',spoilers);
 const birthday=character.birthday?new Intl.DateTimeFormat(i18n.language,{month:'long',...(character.birthday[1]?{day:'numeric' as const}:{})}).format(new Date(2000,character.birthday[0]-1,character.birthday[1]||1)):null;
 const languageCode=person.lang||company.lang;let language=languageCode;try{if(languageCode)language=new Intl.DisplayNames([i18n.language],{type:'language'}).of(languageCode);}catch{/* Keep provider code if the locale cannot resolve it. */}
 const facts=kind==='character'?[[t('profileBirthday'),birthday],[t('profileAge'),character.age==null?null:String(character.age)],[t('profileHeight'),character.height==null?null:`${character.height} cm`],[t('profileBloodType'),character.blood_type?.toUpperCase()]]:[[t('profileLanguage'),language],[t('profileType'),kind==='company'?t('producer_'+company.type,{defaultValue:company.type||''}):null]];
 return <article className={'entity-profile entity-'+kind}>
  <header className="entity-hero">
   <div className="entity-portrait">{kind==='character'?<Picture image={character.image} name={entity.name} hidden={safe}/>:<Avatar name={entity.name} image={person.image} hidden={safe}/>}</div>
   <div className="entity-identity"><p className="eyebrow">{t('profile_'+kind)}</p><h1 ref={heading} tabIndex={-1}>{entity.name}</h1>{entity.original&&entity.original!==entity.name&&<p className="original">{entity.original}</p>}
    {kind==='person'&&initial.name!==entity.name&&<p className="credited-as">{t('creditedAs',{name:initial.name})}</p>}<dl className="entity-facts">{facts.filter(([,value])=>value).map(([label,value])=><div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
    <div className="entity-actions"><button disabled={loading} onClick={refresh}><RefreshCw size={14} className={loading?'spin':''}/>{t('refreshProfile')}</button><a href={`https://vndb.org/${entity.id}`} target="_blank" rel="noreferrer">VNDB <ExternalLink size={14}/></a><button aria-pressed={spoilers} onClick={onSpoilers}>{t(spoilers?'showingSpoilers':'noSpoilers')}</button></div>
   </div>
  </header>
  {loading&&!data&&<p role="status" className="explore-status">{t('loadingProfile')}</p>}{loading&&<button onClick={cancelProvider}>{t('profileCancelProvider')}</button>}
  {(error||data?.partial||data?.stale)&&<div role="alert" className="profile-notice"><p>{error?t(error):data?.warning||t('cachedCredits')}</p><button disabled={loading} onClick={retry}>{t('retry')}</button></div>}
  {canWrite&&data&&<button disabled={loading} onClick={()=>setEditingFields(true)}>{t('entityEditFields')}</button>}
  {canWrite&&editingFields&&<EntityFieldEditor key={'fields:'+entity.id} id={entity.id} name={entity.name} description={entity.description||''} onClose={()=>setEditingFields(false)}/>}
  <EntityTags key={entity.id} id={entity.id} canWrite={canWrite}/>
  <div className="entity-overview"><section className="entity-biography"><h2>{t('profileAbout')}</h2>{biography?<p className="description">{biography}</p>:!loading&&data?<p className="profile-empty">{t(entity.local_fields?.includes('description')?'entityEmptyIntroduction':'sourceBiographyMissing')}</p>:null}
   {aliases.length>0&&<details className="entity-aliases"><summary>{t('knownNames')} · {aliases.length}</summary><div className="tag-list">{aliases.map(alias=><span key={alias} className="pill">{alias}</span>)}</div></details>}
  </section>{links.length>0&&<aside className="entity-links"><h3>{t('profileLinks')}</h3>{links.map(link=><a key={link.url} href={safeExternalUrl(link.url)!} target="_blank" rel="noreferrer">{link.label}<ExternalLink size={13}/></a>)}</aside>}</div>
  {traits.length>0&&<section className="entity-traits"><div className="section-title"><h2>{t('profileTraits')}</h2>{traits.length>18&&<button className="text-button" onClick={()=>setTraitsExpanded(!traitsExpanded)}>{t(traitsExpanded?'showLess':'showAllTraits',{count:traits.length})}</button>}</div><div className="tag-list">{traits.slice(0,traitsExpanded?undefined:18).map(trait=><button className="pill metadata-link" key={trait.id} onClick={()=>onSearch({kind:'trait',value:trait.id,label:`${trait.group_name} · ${trait.name}`})}><small>{trait.group_name}</small>{trait.name}</button>)}</div></section>}
  <section className="entity-credits"><div className="section-title"><h2>{t(kind==='character'?'appearsIn':kind==='company'?'developedWorks':'allCredits')}</h2><span className="profile-count">{ownership.loading||ownership.error?'—':t(data?.more||manualMore?'profileLoadedMore':'profileLoaded',{count:visible.length})}</span></div>
   <div className="tabs credit-tabs">{['all','owned'].map(value=><button key={value} disabled={preparing} aria-pressed={scope===value} className={scope===value?'selected':''} onClick={()=>setScope(value)}>{t(value==='all'?'allWorksProfile':'ownedLoadedWorks')}</button>)}</div>
   {canWrite&&<div className="entity-actions"><button disabled={preparing||loading||manualLoading||incomplete||!!error||!!manualError} onClick={()=>void prepareList()}>{t('listAddEntityScope')}</button>{preparing&&<><span role="status">{t('listPreparingScope',{count:preparedCount})}</span><button onClick={()=>{if(preparation.current){preparation.current.cancelled=true;preparation.current.controller.abort();preparation.current=null;}setPreparing(false);}}>{t('cancel')}</button></>}</div>}
   {manualLoading&&<div><p role="status">{t('profileManualLoading')}</p><button onClick={cancelManual}>{t('profileCancelManual')}</button></div>}
   {manualError&&<div role="alert" className="profile-notice"><p>{t(manualError)}</p><button disabled={manualLoading} onClick={retryManual}>{t('profileManualRetry')}</button></div>}
   {incomplete&&<div role="alert" className="profile-notice"><p>{t(changed?'profileRelationsChanged':'profileRelationsIncomplete')}</p><button disabled={loading||manualLoading} onClick={refresh}>{t('refreshProfile')}</button></div>}
   {listError&&<p role="alert">{listError}</p>}
   {listMembers?.scope===listScope&&<AddToList members={listMembers.members} onClose={()=>setListMembers(null)}/>}
   <OwnedWorksNotice state={ownership}/>
   <div className={kind==='company'?'company-works':'profile-work-list'}>{visible.map(work=>{
    const owned=works.find(owned=>owned.vndb_id===work.id);const roles=(work.va||[]).filter((voice,index,array)=>(kind==='character'?voice.character.id===entity.id:voice.staff.id===entity.id)&&visibleCharacter(voice.character,work.id,spoilers)&&array.findIndex(other=>other.character.id===voice.character.id&&other.staff.id===voice.staff.id&&other.staff.aid===voice.staff.aid&&other.note===voice.note)===index);
    const duties=[...new Set((work.staff||[]).filter(staff=>staff.id===entity.id).map(staff=>[staff.name,t('staff_'+staff.role,{defaultValue:staff.role||t('staffCredit')}),staff.note].filter(Boolean).join(' · ')))];
    return <article className="profile-work" key={work.id}><div className="appearance-row"><Picture image={owned?{url:owned.cover}:work.image} name={work.title} hidden={safe}/><div><h3><button className="metadata-link" onClick={()=>onSearch({kind:'title',value:owned?titleFor(owned,titleMode):work.title,label:owned?titleFor(owned,titleMode):work.title})}>{owned?titleFor(owned,titleMode):work.title}</button></h3>{work.alttitle&&work.alttitle!==work.title&&<p className="credit-original"><button className="metadata-link" onClick={()=>onSearch({kind:'title',value:work.alttitle!,label:work.alttitle!})}>{work.alttitle}</button></p>}<p>{searchableDate(work.released)?<button className="metadata-link" onClick={()=>onSearch({kind:'released',value:work.released!,label:work.released!})}>{work.released}</button>:work.released}</p>{duties.length>0&&<p className="credit-duties">{duties.join(' · ')}</p>}{owned?<button className="text-button" onClick={()=>onWork(owned)}>{t('viewWork')}<ArrowRight size={14}/></button>:<a href={`https://vndb.org/${work.id}`} target="_blank" rel="noreferrer">{t('viewAtVndb')} ↗</a>}</div></div>
     {roles.length>0&&<div className="profile-cast">{roles.map(voice=>kind==='character'?<button className="person-credit" key={`${voice.staff.id}:${voice.staff.aid}:${voice.note}`} onClick={()=>onPerson(voice.staff)}><Avatar name={voice.staff.name} image={voice.staff.image} hidden={safe}/><span><strong>{voice.staff.name}</strong><small>{voice.note||t('japaneseVoice')}</small></span></button>:<button className="profile-character-link" key={`${voice.character.id}:${voice.staff.aid}:${voice.note}`} onClick={()=>onCharacter(voice.character)}><Picture image={voice.character.image} name={voice.character.name} hidden={safe}/><span><strong>{voice.character.name}</strong><small>{[voice.staff.name,voice.note].filter(Boolean).join(' · ')}</small></span><ArrowRight size={14}/></button>)}</div>}
    </article>;
   })}</div>
   {loading&&data&&<p role="status">{t('loadingCredits')}</p>}{!loading&&!manualLoading&&data&&!data.partial&&!error&&!manualError&&!incomplete&&!ownership.loading&&!ownership.error&&!visible.length&&<p>{t('noLoadedCredits')}</p>}
   {data?.more&&!data.partial&&!error&&<button disabled={loading} className="profile-load-more" onClick={loadMore}>{t('loadMoreCredits')}<ArrowRight size={16}/></button>}
   {manualMore&&!manualError&&<button disabled={manualLoading} className="profile-load-more" onClick={loadMoreManual}>{t('profileManualMore')}<ArrowRight size={16}/></button>}
  </section>
 </article>;
}
