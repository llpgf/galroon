import {useOwnedWorks,OwnedWorksNotice} from './useOwnedWorks';
import {useWorkExploration} from './useWorkExploration';
import {useRelationshipRevision} from './relationshipChanges';
import {relationshipChoices} from './relationshipChoices';
import {RelationshipEditor} from './RelationshipEditor';
import {RelatedTags} from './RelatedTags';
import {AddToListButton} from './AddToListButton';
import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {type Work} from './api';
import {Avatar,Picture} from './ProfileVisuals';
import {plainDescription,visibleCharacter,voiceFor,type Person,type Character,type Company} from './exploration';
import {searchableDate,type DiscoveryQuery,type DiscoveryWork} from './discovery';

export function ReferenceWork({active=true,canWrite=false,initial,safe,spoilers,onSpoilers,onCharacter,onPerson,onCompany,onSearch,onCollectionWork}:{active?:boolean;canWrite?:boolean;initial:DiscoveryWork;safe:boolean;spoilers:boolean;onSpoilers:()=>void;onCharacter:(c:Character)=>void;onPerson:(p:Person)=>void;onCompany:(c:Company)=>void;onSearch:(q:DiscoveryQuery)=>void;onCollectionWork:(w:Work)=>void}){
 const {t}=useTranslation();const heading=useRef<HTMLHeadingElement>(null);useEffect(()=>heading.current?.focus({preventScroll:true}),[]);
 const [editingRelations,setEditingRelations]=useState(false);const relationRevision=useRelationshipRevision();
 const {data,loading,error,attempt,refresh}=useWorkExploration(`/vns/${initial.id}/exploration`,spoilers,relationRevision,active);
 const ownership=useOwnedWorks([initial.id],active),works=ownership.works;
 const work={...initial,...data?.vn,developers:data?.vn?.developers||[]};const owned=works.find(w=>w.vndb_id===initial.id);const characters=(data?.characters||[]).filter(c=>visibleCharacter(c,initial.id,spoilers));
 return <article className="reference-work"><p className="eyebrow">{t('discoverySource')}</p><div className="work-detail"><Picture name={work.title||initial.title} image={work.image} hidden={safe} className="detail-cover"/><div className="work-introduction"><h1 ref={heading} tabIndex={-1}>{work.title||initial.title}</h1>{work.alttitle&&<p className="original"><button className="metadata-link" onClick={()=>onSearch({kind:'title',value:work.alttitle!,label:work.alttitle!})}>{work.alttitle}</button></p>}<div className="work-facts"><span className="work-companies">{work.developers?.map(company=><button key={company.id} onClick={()=>onCompany(company)}>{company.name}</button>)}</span>{searchableDate(work.released)&&<button className="metadata-link" onClick={()=>onSearch({kind:'released',value:work.released!,label:work.released!})}>{work.released}</button>}</div><p className="description">{plainDescription(work.description||'',spoilers)}</p><div className="entity-actions">{canWrite&&!ownership.loading&&!ownership.error&&<AddToListButton member={{work_key:owned?'local:'+owned.id:'vndb:'+initial.id,title:work.title||initial.title}}/>}{owned&&<button onClick={()=>onCollectionWork(owned)}>{t('discoveryOpenCollection')}</button>}<a href={`https://vndb.org/${initial.id}`} target="_blank" rel="noreferrer">VNDB ↗</a><button aria-pressed={spoilers} onClick={onSpoilers}>{t(spoilers?'showingSpoilers':'noSpoilers')}</button></div></div></div>
 <OwnedWorksNotice state={ownership}/>
 {loading&&<p role="status">{t('loadingCredits')}</p>}{(error||data?.warning)&&<div className="profile-notice" role="alert"><p>{error||data?.warning}</p><button disabled={loading} onClick={refresh}>{t('retry')}</button></div>}
 <section className="explore-section"><h2>{t('characters')}</h2><div className="character-grid">{characters.map(character=><article className="character-card" key={character.id}><button className="character-link" onClick={()=>onCharacter(character)}><Picture name={character.name} image={character.image} hidden={safe}/><strong>{character.name}</strong></button>{data&&voiceFor(data,character.id).map(voice=><button className="person-credit" key={`${voice.staff.id}:${voice.staff.aid}:${voice.note}`} onClick={()=>onPerson(voice.staff)}><Avatar name={voice.staff.name} hidden={safe}/><span><strong>{voice.staff.name}</strong><small>{voice.note||t('japaneseVoice')}</small></span></button>)}</article>)}</div></section>
 <section className="explore-section"><h2>{t('staff')}</h2><div className="staff-grid">{data?.vn?.staff?.map((person,index)=><button className="person-credit" key={`${person.id}:${index}`} onClick={()=>onPerson(person)}><Avatar name={person.name} hidden={safe}/><span><strong>{person.name}</strong><small>{[t('staff_'+person.role,{defaultValue:person.role||t('staffCredit')}),person.note].filter(Boolean).join(' · ')}</small></span></button>)}</div></section>
 {canWrite&&data&&<button disabled={loading} onClick={()=>setEditingRelations(true)}>{t('relationEdit')}</button>}
 {editingRelations&&canWrite&&<RelationshipEditor key={initial.id} id={initial.id} choices={relationshipChoices(data,initial.id,spoilers)} onClose={()=>setEditingRelations(false)} onSaved={()=>{}}/>}
 <RelatedTags workId={initial.id} reference active={active&&!loading} refreshKey={attempt+relationRevision+(data?.fetched_at||0)} onOpen={reason=>{const entity={id:reason.entity_id,name:reason.name};if(reason.kind==='person')onPerson(entity);else if(reason.kind==='character')onCharacter(entity);else onCompany(entity);}}/>
 </article>;
}
