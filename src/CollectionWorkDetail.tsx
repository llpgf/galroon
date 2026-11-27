import {VndbWork} from './VndbWork';
import {BangumiWork} from './BangumiWork';
import {VndbPairing} from './VndbPairing';
import {useEffect,useState,type ComponentProps,type FormEvent} from 'react';
import {useTranslation} from 'react-i18next';
import {Check,Copy,FolderOpen,Heart,MoreHorizontal,PencilLine,Combine,RefreshCw,Star} from 'lucide-react';
import {api,isDesktop,type Work,type Resource,type Root} from './api';
import {WorkExperience} from './WorkExperience';
import {EditionsPanel} from './EditionsPanel';
import {AddToListButton} from './AddToListButton';
import {useWorkAssets} from './useWorkAssets';
import {copyText,localFolder,primaryResourceForWork,resolveResourcePath,revealInFolder,saveLocalRoot,type ResolvedResourcePath} from './localFolder';

type Conflict={field:string;mine:unknown;provider:unknown};
const shown=(v:unknown)=>Array.isArray(v)?v.join(', '):String(v??'');
type Props=Omit<ComponentProps<typeof WorkExperience>,'actions'|'children'|'primaryAction'|'extraAnchors'|'heroNotice'>&{
 roots:Root[];busy:boolean;
 act:(fn:()=>Promise<unknown>)=>Promise<void>;refresh:()=>Promise<void>;
 onEdit:(work:Work)=>void;onGroup:(work:Work)=>void;
 onReferences:(resource:Resource)=>void;onAcquire:(resource:Resource)=>void;onOrganize:(resource:Resource)=>void;
};
/** Both collection and retained List exploration use the same local work surface. */
export function CollectionWorkDetail({roots,busy,act,refresh,onEdit,onGroup,onReferences,onAcquire,onOrganize,...experience}:Props){
 const {t}=useTranslation(),{work,canWrite}=experience;
 const assets=useWorkAssets(work);
 // Keep unsaved text and its original CAS revision across background refreshes.
 const [draft,setDraft]=useState<{notes:string;revision:number}|null>(null);
 const [rating,setRating]=useState(work.personal_rating==null?'':String(work.personal_rating/10));
 const [ratingBusy,setRatingBusy]=useState(false),[ratingError,setRatingError]=useState('');
 useEffect(()=>{setRating(work.personal_rating==null?'':String(work.personal_rating/10));},[work.id,work.personal_rating]);
 // Refresh keeps manual values; where VNDB now says something else, the person chooses per field.
 const [conflicts,setConflicts]=useState<{revision:number;items:Conflict[]}|null>(null);
 const [mapVersion,setMapVersion]=useState(0);
 const [mappingPrompt,setMappingPrompt]=useState<{resolved:ResolvedResourcePath;resource:Resource}|null>(null);
 const [mapInput,setMapInput]=useState('');
 const [openError,setOpenError]=useState('');
 const [copiedHero,setCopiedHero]=useState(false);

 const resolve=(item:Conflict,useProvider:boolean)=>void act(async()=>{let revision=conflicts!.revision;if(useProvider)revision=(await api<{revision:number}>(`/works/${work.id}/metadata`,{revision,reset:[item.field]})).revision;const items=conflicts!.items.filter(c=>c.field!==item.field);setConflicts(items.length?{revision,items}:null);await refresh();});
 const update=(patch:object)=>void act(()=>api(`/works/${work.id}`,{revision:work.revision,...patch}));

 const savedRatingStr=work.personal_rating==null?'':String(work.personal_rating/10);
 const ratingDirty=rating.trim()!==savedRatingStr;
 async function saveRating(e:FormEvent){
  e.preventDefault();
  setRatingBusy(true);setRatingError('');
  try{
   await api('/vndb_sync/rating',{work_id:work.id,revision:work.revision,rating:rating.trim()===''?null:Math.round(Number(rating)*10)});
   await refresh();
  }catch(err){setRatingError(err instanceof Error?err.message:String(err));}
  finally{setRatingBusy(false);}
 }

 const primaryResource=assets.data?primaryResourceForWork(work.id,work.preferred_release_id,assets.data.resources,roots):null;
 // Touch mapVersion so path resolution re-evaluates after a remote-root mapping is saved.
 const primaryResolved=primaryResource?(void mapVersion,resolveResourcePath(primaryResource,roots)):null;

 async function openPrimaryFolder(){
  if(!primaryResource||!primaryResolved)return;
  setOpenError('');
  if(primaryResolved.openPath){
   try{await revealInFolder(primaryResolved.openPath);}
   catch(err){setOpenError(err instanceof Error?err.message:String(err));}
   return;
  }
  if(primaryResolved.needsMapping){
   setMapInput(primaryResolved.mappedRoot);
   setMappingPrompt({resolved:primaryResolved,resource:primaryResource});
  }
 }

 async function copyPrimaryPath(){
  const target=primaryResolved?.openPath||primaryResolved?.corePath||'';
  if(!target)return;
  setOpenError('');
  if(await copyText(target)){
   setCopiedHero(true);
   setTimeout(()=>setCopiedHero(false),1800);
  }else{
   setOpenError(t('copyFailed'));
  }
 }

 async function saveHeroMapping(openAfter:boolean){
  if(!mappingPrompt)return;
  saveLocalRoot(mappingPrompt.resolved.rootId,mapInput);
  setMapVersion(v=>v+1);
  const next=resolveResourcePath(mappingPrompt.resource,roots);
  setMappingPrompt(null);
  if(openAfter&&next.openPath){
   try{await revealInFolder(next.openPath);}
   catch(err){setOpenError(err instanceof Error?err.message:String(err));}
  }
 }

 const primaryAction=primaryResource&&primaryResolved?(isDesktop
  ?<button className="primary" onClick={()=>void openPrimaryFolder()} title={primaryResolved.openPath||primaryResolved.corePath}><FolderOpen size={16}/>{t('openFolder')}</button>
  :<button className="primary" onClick={()=>void copyPrimaryPath()} title={primaryResolved.corePath}>{copiedHero?<Check size={16}/>:<Copy size={16}/>}{t(copiedHero?'copied':'copyPath')}</button>):undefined;

 const heroNotice=(openError||ratingError||mappingPrompt)?<div className="hero-notices">
  {openError&&<p className="error-text" role="alert">{openError}</p>}
  {ratingError&&<p className="error-text" role="alert">{ratingError}</p>}
  {mappingPrompt&&<div className="local-map-bar" role="region" aria-label={t('localFolderMapTitle')}>
   <p><strong>{t('localFolderMapTitle')}</strong> · {t('localFolderMapHint',{root:mappingPrompt.resolved.rootLabel||mappingPrompt.resolved.rootCorePath,path:mappingPrompt.resolved.rootCorePath})}</p>
   <div className="inline">
    <input value={mapInput} onChange={e=>setMapInput(e.target.value)} placeholder={t('localFolderMapPlaceholder')} aria-label={t('localFolderMapTitle')}/>
    <button type="button" onClick={()=>void localFolder().then(p=>{if(p)setMapInput(p);})}><FolderOpen size={15}/>{t('browse')}</button>
    <button type="button" className="primary" disabled={!mapInput.trim()} onClick={()=>void saveHeroMapping(true)}>{t('localFolderMapSaveOpen')}</button>
    <button type="button" onClick={()=>setMappingPrompt(null)}>{t('cancel')}</button>
   </div>
  </div>}
 </div>:undefined;

 const notesDirty=draft!==null&&draft.notes!==work.notes;
 const extraAnchors=[{id:'work-files',label:t('editionsFiles')},{id:'work-notes',label:t('notes')}];

 // A new VNDB pairing changes what the exploration sections load.
 return <WorkExperience key={work.vndb_id||''} {...experience} primaryAction={primaryAction} extraAnchors={extraAnchors} heroNotice={heroNotice} actions={canWrite?<fieldset className="inline detail-actions">
  <select className="status-select" disabled={busy} aria-label={t('status')} value={work.status} onChange={e=>update({status:e.target.value})}>{['none','backlog','playing','completed','on_hold','dropped'].map(s=><option value={s} key={s}>{t(s)}</option>)}</select>
  <button className="icon-toggle" disabled={busy} aria-pressed={work.favorite} aria-label={t('favorite')} title={t('favorite')} onClick={()=>update({favorite:!work.favorite})}><Heart size={16} fill={work.favorite?'currentColor':'none'}/></button>
  <form className="hero-rating" onSubmit={e=>void saveRating(e)}>
   <label><Star size={14} aria-hidden="true"/><span className="visually-hidden">{t('vnsMyRating')}</span><input type="number" min="1" max="10" step="0.1" value={rating} onChange={e=>setRating(e.target.value)} disabled={busy||ratingBusy} placeholder={t('vnsUnrated')}/></label>
   <button type="submit" disabled={busy||ratingBusy} className={ratingDirty?'primary':undefined}>{t('vnsSaveRating')}</button>
  </form>
  <AddToListButton member={{work_key:'local:'+work.id,title:work.title}}/>
  <details className="toolbar-menu"><summary aria-label={t('moreOptions')} title={t('moreOptions')}><MoreHorizontal size={18}/></summary><div className="toolbar-menu-panel">
   <button disabled={busy} onClick={()=>onEdit(work)}><PencilLine size={15}/>{t('editMetadata')}</button><button disabled={busy} onClick={()=>onGroup(work)}><Combine size={15}/>{t('groupingTitle')}</button>
   {work.vndb_id&&<button disabled={busy} onClick={()=>void act(async()=>{const result=await api<{revision:number;conflicts?:Conflict[]}>(`/works/${work.id}/refresh`,{revision:work.revision});setConflicts(result.conflicts?.length?{revision:result.revision,items:result.conflicts}:null);})}><RefreshCw size={15}/>{t('refreshMetadata')}</button>}
  </div></details>
 </fieldset>:<span className="pill">{t(work.status)}</span>}>
  {!!work.same_work?.length&&<section className="section same-work"><h2>{t('sameWorkCards')}</h2><p>{t('sameWorkHint')}</p><div className="inline">{work.same_work.map(other=><button key={other.id} disabled={busy} onClick={()=>void act(async()=>experience.onWork(await api<Work>(`/works/${other.id}`)))}>{other.original_title||other.title}{other.released?` · ${other.released}`:''}{other.vndb_release?` · ${t('remakeCard')}`:''}</button>)}</div></section>}
  {conflicts&&<section className="section refresh-conflicts" role="region" aria-label={t('refreshConflicts')}><h2>{t('refreshConflicts')}</h2><p>{t('refreshConflictsHint')}</p>{conflicts.items.map(item=><div className="plan-item" key={item.field}><strong>{t('field_'+item.field)}</strong><p>{t('conflictMine')}: {shown(item.mine)}</p><p>{t('conflictProvider')}: {shown(item.provider)}</p><div className="inline"><button disabled={busy} onClick={()=>resolve(item,false)}>{t('keepMine')}</button><button className="primary" disabled={busy} onClick={()=>resolve(item,true)}>{t('useProvider')}</button></div></div>)}</section>}
  <EditionsPanel readOnly={!canWrite} work={work} roots={roots} assets={assets} mapVersion={mapVersion} onMapChanged={()=>setMapVersion(v=>v+1)} onPreferenceChanged={refresh} onChanged={refresh} onReferences={onReferences} onAcquire={onAcquire} onOrganize={onOrganize}/>
  <section id="work-notes" className="section notes-section">
   <div className="section-title"><h2>{t('notes')}</h2>{notesDirty&&<span className="pill notes-dirty">{t('unsavedChanges')}</span>}</div>
   <div className="notes-card">
    <textarea aria-label={t('notes')} readOnly={!canWrite} placeholder={t('notesPlaceholder')} value={draft?.notes??work.notes} onChange={e=>setDraft({notes:e.target.value,revision:draft?.revision??work.revision})}/>
    {canWrite&&<div className="inline notes-actions"><button className={notesDirty?'primary':undefined} disabled={busy||!notesDirty} onClick={()=>void act(async()=>{await api(`/works/${work.id}`,{revision:draft?.revision??work.revision,notes:draft?.notes??work.notes});setDraft(null);})}>{t('save')}</button>{notesDirty&&<button disabled={busy} onClick={()=>setDraft(null)}>{t('cancel')}</button>}</div>}
   </div>
  </section>
  {canWrite&&!work.vndb_id&&work.resources===0&&<VndbPairing key={work.id+':pair:'+work.revision} work={work} onChanged={refresh}/>}
  {canWrite&&<div className="sync-grid">
   <VndbWork key={work.id+':vndb:'+work.revision} work={work} onChanged={refresh} onHidden={experience.onBack}/>
   <BangumiWork key={work.id+':'+work.bangumi_id} work={work} onChanged={refresh} onHidden={experience.onBack}/>
  </div>}
 </WorkExperience>;
}
