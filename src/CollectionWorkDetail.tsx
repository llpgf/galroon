import {useState,type ComponentProps} from 'react';
import {useTranslation} from 'react-i18next';
import {Heart} from 'lucide-react';
import {api,type Work,type Resource,type Root} from './api';
import {WorkExperience} from './WorkExperience';
import {EditionsPanel} from './EditionsPanel';
import {AddToListButton} from './AddToListButton';

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
 const update=(patch:object)=>void act(()=>api(`/works/${work.id}`,{revision:work.revision,...patch}));
 return <WorkExperience {...experience} actions={canWrite?<fieldset className="inline detail-actions">
  <AddToListButton member={{work_key:'local:'+work.id,title:work.title}}/>
  <select disabled={busy} aria-label={t('status')} value={work.status} onChange={e=>update({status:e.target.value})}>{['backlog','playing','completed','on_hold','dropped'].map(s=><option value={s} key={s}>{t(s)}</option>)}</select>
  <button disabled={busy} onClick={()=>update({favorite:!work.favorite})}><Heart size={16} fill={work.favorite?'currentColor':'none'}/>{t('favorite')}</button>
  <button disabled={busy} onClick={()=>onEdit(work)}>{t('editMetadata')}</button><button disabled={busy} onClick={()=>onGroup(work)}>{t('groupingTitle')}</button>
  {work.vndb_id&&<button disabled={busy} onClick={()=>void act(()=>api(`/works/${work.id}/refresh`,{revision:work.revision}))}>{t('refreshMetadata')}</button>}
 </fieldset>:<p>{t('status')}: {t(work.status)}</p>}>
  <EditionsPanel readOnly={!canWrite} work={work} roots={roots} onPreferenceChanged={refresh} onChanged={refresh} onReferences={onReferences} onAcquire={onAcquire} onOrganize={onOrganize}/>
  <section className="section"><h2>{t('notes')}</h2><textarea aria-label={t('notes')} readOnly={!canWrite} value={draft?.notes??work.notes} onChange={e=>setDraft({notes:e.target.value,revision:draft?.revision??work.revision})}/>{canWrite&&<button disabled={busy} onClick={()=>void act(async()=>{await api(`/works/${work.id}`,{revision:draft?.revision??work.revision,notes:draft?.notes??work.notes});setDraft(null);})}>{t('save')}</button>}</section>
 </WorkExperience>;
}
