import {useState,type ComponentProps} from 'react';
import {useTranslation} from 'react-i18next';
import {Heart,MoreHorizontal,PencilLine,Combine,RefreshCw} from 'lucide-react';
import {api,type Work,type Resource,type Root} from './api';
import {WorkExperience} from './WorkExperience';
import {EditionsPanel} from './EditionsPanel';
import {AddToListButton} from './AddToListButton';

type Conflict={field:string;mine:unknown;provider:unknown};
const shown=(v:unknown)=>Array.isArray(v)?v.join(', '):String(v??'');
type Props=Omit<ComponentProps<typeof WorkExperience>,'actions'|'children'>&{
 roots:Root[];busy:boolean;
 act:(fn:()=>Promise<unknown>)=>Promise<void>;refresh:()=>Promise<void>;
 onEdit:(work:Work)=>void;onGroup:(work:Work)=>void;
 onReferences:(resource:Resource)=>void;onAcquire:(resource:Resource)=>void;onOrganize:(resource:Resource)=>void;
};
/** Both collection and retained List exploration use the same local work surface. */
export function CollectionWorkDetail({roots,busy,act,refresh,onEdit,onGroup,onReferences,onAcquire,onOrganize,...experience}:Props){
 const {t}=useTranslation(),{work,canWrite}=experience;
 // Keep unsaved text and its original CAS revision across background refreshes.
 const [draft,setDraft]=useState<{notes:string;revision:number}|null>(null);
 // Refresh keeps manual values; where VNDB now says something else, the person chooses per field.
 const [conflicts,setConflicts]=useState<{revision:number;items:Conflict[]}|null>(null);
 const resolve=(item:Conflict,useProvider:boolean)=>void act(async()=>{let revision=conflicts!.revision;if(useProvider)revision=(await api<{revision:number}>(`/works/${work.id}/metadata`,{revision,reset:[item.field]})).revision;const items=conflicts!.items.filter(c=>c.field!==item.field);setConflicts(items.length?{revision,items}:null);await refresh();});
 const update=(patch:object)=>void act(()=>api(`/works/${work.id}`,{revision:work.revision,...patch}));
 return <WorkExperience {...experience} actions={canWrite?<fieldset className="inline detail-actions">
  <select className="status-select" disabled={busy} aria-label={t('status')} value={work.status} onChange={e=>update({status:e.target.value})}>{['backlog','playing','completed','on_hold','dropped'].map(s=><option value={s} key={s}>{t(s)}</option>)}</select>
  <button className="icon-toggle" disabled={busy} aria-pressed={work.favorite} aria-label={t('favorite')} title={t('favorite')} onClick={()=>update({favorite:!work.favorite})}><Heart size={16} fill={work.favorite?'currentColor':'none'}/></button>
  <AddToListButton member={{work_key:'local:'+work.id,title:work.title}}/>
  <details className="toolbar-menu"><summary aria-label={t('moreOptions')} title={t('moreOptions')}><MoreHorizontal size={18}/></summary><div className="toolbar-menu-panel">
   <button disabled={busy} onClick={()=>onEdit(work)}><PencilLine size={15}/>{t('editMetadata')}</button><button disabled={busy} onClick={()=>onGroup(work)}><Combine size={15}/>{t('groupingTitle')}</button>
   {work.vndb_id&&<button disabled={busy} onClick={()=>void act(async()=>{const result=await api<{revision:number;conflicts?:Conflict[]}>(`/works/${work.id}/refresh`,{revision:work.revision});setConflicts(result.conflicts?.length?{revision:result.revision,items:result.conflicts}:null);})}><RefreshCw size={15}/>{t('refreshMetadata')}</button>}
  </div></details>
 </fieldset>:<span className="pill">{t(work.status)}</span>}>
  {!!work.same_work?.length&&<section className="section same-work"><h2>{t('sameWorkCards')}</h2><p>{t('sameWorkHint')}</p><div className="inline">{work.same_work.map(other=><button key={other.id} disabled={busy} onClick={()=>void act(async()=>experience.onWork(await api<Work>(`/works/${other.id}`)))}>{other.original_title||other.title}{other.released?` · ${other.released}`:''}{other.vndb_release?` · ${t('remakeCard')}`:''}</button>)}</div></section>}
  {conflicts&&<section className="section refresh-conflicts" role="region" aria-label={t('refreshConflicts')}><h2>{t('refreshConflicts')}</h2><p>{t('refreshConflictsHint')}</p>{conflicts.items.map(item=><div className="plan-item" key={item.field}><strong>{t('field_'+item.field)}</strong><p>{t('conflictMine')}: {shown(item.mine)}</p><p>{t('conflictProvider')}: {shown(item.provider)}</p><div className="inline"><button disabled={busy} onClick={()=>resolve(item,false)}>{t('keepMine')}</button><button className="primary" disabled={busy} onClick={()=>resolve(item,true)}>{t('useProvider')}</button></div></div>)}</section>}
  <EditionsPanel readOnly={!canWrite} work={work} roots={roots} onPreferenceChanged={refresh} onChanged={refresh} onReferences={onReferences} onAcquire={onAcquire} onOrganize={onOrganize}/>
  <section className="section"><h2>{t('notes')}</h2><textarea aria-label={t('notes')} readOnly={!canWrite} value={draft?.notes??work.notes} onChange={e=>setDraft({notes:e.target.value,revision:draft?.revision??work.revision})}/>{canWrite&&<button disabled={busy} onClick={()=>void act(async()=>{await api(`/works/${work.id}`,{revision:draft?.revision??work.revision,notes:draft?.notes??work.notes});setDraft(null);})}>{t('save')}</button>}</section>
 </WorkExperience>;
}
