import {useNames} from './display';
import {useOwnedWorks,OwnedWorksNotice} from './useOwnedWorks';
import {useWorkExploration} from './useWorkExploration';
import {useRelationshipRevision} from './relationshipChanges';
import {relationshipChoices} from './relationshipChoices';
import {RelationshipEditor} from './RelationshipEditor';
import {RelatedTags} from './RelatedTags';
import {DiscoverySearch} from './DiscoverySearch';
import {ReferenceWork} from './ReferenceWork';
import type {DiscoveryQuery,DiscoveryWork} from './discovery';
import {Avatar,Picture} from './ProfileVisuals';
import {EntityProfile} from './EntityProfile';
import {navigationOrigin,restoreNavigationFocus} from './navigationFocus';
import {useRef,useState,type ReactNode} from 'react';
import {ArrowLeft,RefreshCw,ExternalLink,FolderOpen,Eye,EyeOff,Users} from 'lucide-react';
import {useTranslation} from 'react-i18next';
import {type Work} from './api';
import {titleFor,tagNames,coverTone} from './library';
import {visibleCharacter,voiceFor,plainDescription,type Person,type Character,type Company} from './exploration';

function Credit({person,role,onOpen,hidden}:{person:Person;role:string;onOpen:(p:Person)=>void;hidden:boolean}){
 const names=useNames();
 return <button className="person-credit" onClick={()=>onOpen(person)}><Avatar name={person.name} image={person.image} hidden={hidden}/><span><strong>{names.name(person)}</strong><small>{role}</small></span></button>;
}
type Profile={kind:'character';character:Character}|{kind:'person';person:Person}|{kind:'company';company:Company}|{kind:'search';query:DiscoveryQuery}|{kind:'work';work:DiscoveryWork};
export function WorkExperience({canWrite=false,backLabel="myCollection",initialSearch,work,safe,titleMode,actions,children,onBack,onWork}:{canWrite?:boolean;backLabel?:string;initialSearch?:DiscoveryQuery|null;work:Work;safe:boolean;titleMode:'display'|'original';actions:ReactNode;children:ReactNode;onBack:()=>void;onWork:(w:Work)=>void}){
 const {t}=useTranslation();const names=useNames();const [spoilers,setSpoilers]=useState(false),[allStaff,setAllStaff]=useState(true),[expandedStaff,setExpandedStaff]=useState(false),[frames,setFrames]=useState<{view:Profile;y:number;origin:HTMLElement|null}[]>(()=>initialSearch?[{view:{kind:'search',query:initialSearch},y:0,origin:null}]:[]);
 const [editingRelations,setEditingRelations]=useState(false),[expanded,setExpanded]=useState(false);const relationRevision=useRelationshipRevision();
 const rootScroll=useRef(0),container=useRef<HTMLDivElement>(null);
 const {data,error,loading,attempt,refresh}=useWorkExploration(`/works/${work.id}/exploration`,spoilers,relationRevision);
 const ownership=useOwnedWorks((data?.vn?.relations||[]).map(work=>work.id),frames.length===0);const works=ownership.works;
 function open(next:Profile){const y=window.scrollY,origin=navigationOrigin(container.current);if(!frames.length)rootScroll.current=y;setFrames(previous=>[...previous.map((frame,index)=>index===previous.length-1?{...frame,y}:frame),{view:next,y:0,origin}]);requestAnimationFrame(()=>window.scrollTo(0,0));}
 function back(){if(initialSearch&&frames.length===1){onBack();return;}const y=frames.length>1?frames[frames.length-2].y:rootScroll.current,origin=frames.at(-1)?.origin||null;setFrames(previous=>previous.slice(0,-1));requestAnimationFrame(()=>{window.scrollTo(0,y);restoreNavigationFocus(origin,container.current);});}
 const showSearch=(query:DiscoveryQuery)=>open({kind:'search',query});
 const showReference=(work:DiscoveryWork)=>open({kind:'work',work});
 const showPerson=(person:Person)=>open({kind:'person',person});
 const characters=(data?.characters||[]).filter(c=>visibleCharacter(c,work.vndb_id||'',spoilers)).sort((a,b)=>{const rank=(c:Character)=>({main:0,primary:1,side:2,appears:3}[c.vns?.find(v=>v.id===work.vndb_id)?.role||'']??4);return rank(a)-rank(b)||a.name.localeCompare(b.name);});
 const credits=(data?.vn?.staff||[]).filter(p=>allStaff||p.eid==null);const seen=new Set<string>();const staff=credits.filter(p=>{const key=`${p.id}:${p.aid}:${p.role}:${p.eid}:${p.note}`;if(seen.has(key))return false;seen.add(key);return true;});
 const jump=(id:string)=>container.current?.querySelector<HTMLElement>('#'+id)?.scrollIntoView({behavior:window.matchMedia('(prefers-reduced-motion: reduce)').matches?'auto':'smooth',block:'start'});
 const description=plainDescription(work.description,spoilers);
 // Works without a VNDB link have no credits to explore, so one notice replaces three empty sections.
 const local=!work.vndb_id;
 const role=(p:Person)=>t('staff_'+p.role,{defaultValue:p.role||t('staffCredit')});
 return <div className="work-experience" ref={container}>
  <div className="exploration-frame" hidden={frames.length>0}>
   <header className="work-hero"><Picture image={{url:work.cover}} name="" hidden={safe} className="work-hero-backdrop"/>
   <button className="back-button" onClick={onBack}><ArrowLeft size={16}/>{t(backLabel)}</button>
   <div className="work-detail"><Picture image={{url:work.cover}} name={work.title} hidden={safe} className={'detail-cover cover-'+coverTone(work.id)} fallback={<span className="cover-title" aria-hidden="true">{work.original_title||work.title}</span>}/><div className="work-introduction"><h1 tabIndex={-1}>{titleFor(work,titleMode)}</h1>{work.original_title!==work.title&&<p className="original">{work.original_title}</p>}<p className="work-facts"><span className="work-companies">{data?.vn?.developers?.length?data.vn.developers.map(company=><button key={company.id} onClick={()=>open({kind:'company',company})}>{names.name(company)}</button>):!work.vndb_id&&work.developer||t('unknown')}</span><span>{work.released||t('unknown')}</span>{work.vndb_id&&<a href={`https://vndb.org/${work.vndb_id}`} target="_blank" rel="noreferrer">VNDB <ExternalLink size={12}/></a>}</p>{description&&<><p className={'description'+(expanded?' expanded':'')}>{description}</p>{description.length>360&&<button className="text-button description-toggle" aria-expanded={expanded} onClick={()=>setExpanded(!expanded)}>{t(expanded?'showLess':'readMore')}</button>}</>}{!description&&<p className="description muted">{t('noDescription')}</p>}<div className="work-actions"><button className="primary" onClick={()=>jump('work-files')}><FolderOpen size={16}/>{t('editionsFiles')}</button>{actions}<button aria-pressed={spoilers} title={t(spoilers?'showingSpoilers':'noSpoilers')} onClick={()=>setSpoilers(!spoilers)}>{spoilers?<Eye size={16}/>:<EyeOff size={16}/>}{t(spoilers?'showingSpoilers':'noSpoilers')}</button></div></div></div>
   </header>
   {!local&&<nav className="work-anchors" aria-label={t('onThisPage')}>{['characters','staff','related'].map(s=><button key={s} onClick={()=>jump('work-'+s)}>{t(s)}</button>)}</nav>}
   {!loading&&!local&&<button className="text-button credits-refresh" onClick={refresh}><RefreshCw size={14}/>{t('refreshCredits')}</button>}
   {loading&&!local&&<p className="explore-status" role="status"><RefreshCw className="spin" size={16}/>{t('loadingCredits')}</p>}
   {error&&<div className="explore-status" role="alert"><span>{error}</span><button onClick={refresh}>{t('retry')}</button></div>}
   {(data?.stale||data?.partial)&&<p className="explore-status">{t('cachedCredits')} {data.warning}</p>}
   {local?<section id="work-credits" className="explore-section explore-local"><Users size={22} strokeWidth={1.6} aria-hidden="true"/><div><h2>{t('localCreditsTitle')}</h2><p>{t('localCreditsHint')}</p></div></section>:<>
   <section id="work-characters" className="explore-section"><h2>{t('characters')}</h2><p>{t('pairedVoices')}</p><div className="character-grid">{characters.map(c=><article className="character-card" key={c.id}>{(()=>{const role=c.vns?.find(v=>v.id===work.vndb_id)?.role;return <button className="character-link" onClick={()=>open({kind:'character',character:c})}><Picture image={c.image} hidden={safe} name={c.name}/><span className="character-name"><strong>{names.name(c)}</strong>{role&&<small className={'character-role role-'+role}>{t('charRole_'+role,{defaultValue:role})}</small>}</span></button>;})()}{data&&voiceFor(data,c.id).length>0&&<div className="character-voices"><span className="cv-label">CV</span>{voiceFor(data,c.id).map(v=><Credit key={`${v.staff.id}:${v.staff.aid}:${v.note}`} person={v.staff} role={v.note||t('japaneseVoice')} onOpen={showPerson} hidden={safe}/>)}</div>}</article>)}</div>{!loading&&!error&&!data?.partial&&!characters.length&&<p>{t(data?.local?'localCreditsHint':'noCharacters')}</p>}{data?.more&&<p>{t('charactersLimited')} <a href={`https://vndb.org/${work.vndb_id}/chars`} target="_blank" rel="noreferrer">VNDB <ExternalLink size={12}/></a></p>}</section>
   <section id="work-staff" className="explore-section"><div className="section-title"><h2>{t('staff')}</h2><label className="checkbox-label"><input type="checkbox" checked={allStaff} onChange={e=>{setAllStaff(e.target.checked);setExpandedStaff(false);}}/>{t('allEditionCredits')}</label></div><div className="staff-grid">{staff.slice(0,expandedStaff?undefined:12).map(p=><Credit key={`${p.id}:${p.aid}:${p.role}:${p.eid}:${p.note}`} person={p} role={[role(p),p.note].filter(Boolean).join(' · ')} onOpen={showPerson} hidden={safe}/>)}</div>{!loading&&!error&&!staff.length&&<p>{t('noStaff')}</p>}{staff.length>12&&<button className="text-button" onClick={()=>setExpandedStaff(!expandedStaff)}>{t(expandedStaff?'showLess':'showAllCredits',{count:staff.length})}</button>}</section>
   <section id="work-related" className="explore-section"><div className="section-title"><h2>{t('related')}</h2>{canWrite&&work.vndb_id&&data&&<button disabled={loading} onClick={()=>setEditingRelations(true)}>{t('relationEdit')}</button>}</div><OwnedWorksNotice state={ownership}/><div className="related-grid">{!ownership.loading&&!ownership.error&&(data?.vn?.relations||[]).map(r=>{const owned=works.find(w=>w.vndb_id===r.id);const content=<><Picture image={owned?{url:owned.cover}:r.image} hidden={safe} name={r.title}/><strong>{owned?titleFor(owned,titleMode):r.title}</strong><small>{t('relation_'+r.relation,{defaultValue:r.relation||t('related')})} · {t(owned?'inCollection':'externalReference')}</small></>;return owned?<button className="related-card" key={r.id} onClick={()=>onWork(owned)}>{content}</button>:<a className="related-card" key={r.id} href={`https://vndb.org/${r.id}`} target="_blank" rel="noreferrer">{content}</a>;})}</div>{!loading&&!error&&!data?.vn?.relations?.length&&<p>{t('noRelated')}</p>}</section>
   </>}
   {tagNames(work.tags).length>0&&<details className="work-tags"><summary>{t('workTags')}</summary><div className="tag-list">{tagNames(work.tags).map(tag=><span className="pill" key={tag}>{tag}</span>)}</div></details>}
   {editingRelations&&canWrite&&work.vndb_id&&<RelationshipEditor key={work.vndb_id} id={work.vndb_id} choices={relationshipChoices(data,work.vndb_id,spoilers)} onClose={()=>setEditingRelations(false)} onSaved={()=>{}}/>}
   <RelatedTags workId={work.id} active={!frames.length&&!loading} refreshKey={attempt+relationRevision+(data?.fetched_at||0)} onOpen={reason=>{const entity={id:reason.entity_id,name:reason.name};if(reason.kind==='person')showPerson(entity);else if(reason.kind==='character')open({kind:'character',character:entity});else open({kind:'company',company:entity});}}/>
   <div id="work-files">{children}</div>
  </div>
  {frames.map(({view:profile},index)=><div className="exploration-frame" hidden={index!==frames.length-1} key={index}><button className="back-button" onClick={back}><ArrowLeft size={16}/>{t('back')}</button>{profile.kind==='search'?<DiscoverySearch canWrite={canWrite} visible={index===frames.length-1} query={profile.query} safe={safe} spoilers={spoilers} onSpoilers={()=>setSpoilers(!spoilers)} onCharacter={character=>open({kind:'character',character})} onWork={showReference}/>:profile.kind==='work'?<ReferenceWork active={index===frames.length-1} canWrite={canWrite} initial={profile.work} safe={safe} spoilers={spoilers} onSpoilers={()=>setSpoilers(!spoilers)} onCharacter={character=>open({kind:'character',character})} onPerson={showPerson} onCompany={company=>open({kind:'company',company})} onSearch={showSearch} onCollectionWork={onWork}/>:<EntityProfile active={index===frames.length-1} canWrite={canWrite} kind={profile.kind} initial={profile.kind==='character'?profile.character:profile.kind==='person'?profile.person:profile.company} safe={safe} spoilers={spoilers} onSpoilers={()=>setSpoilers(!spoilers)} onWork={onWork} onPerson={showPerson} onCharacter={character=>open({kind:'character',character})} onSearch={showSearch} titleMode={titleMode}/>}</div>)}
 </div>;
}

