import {useTagMembership,type TagMembershipBase} from './useTagMembership';
import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {emptyFilters,titleFor} from './library';
import {useCollectionPage} from './useCollectionPage';

export function TagWorkPicker({selected,onChange,titleMode,disabled,tag}:{selected:string[];onChange:(ids:string[])=>void;titleMode:'display'|'original';disabled:boolean;tag?:TagMembershipBase}){
 const {t,i18n}=useTranslation(),[query,setQuery]=useState('');
 const page=useCollectionPage({filters:emptyFilters,query,status:'all',sort:'title',titleMode,locale:i18n.language},0);
 const rows=page.data?.items||[],chosen=new Set(selected),membership=useTagMembership(tag,rows.map(w=>w.id)),members=new Set(membership.members);
 return <section>
  <label>{t('findWorks')}<input type="search" value={query} disabled={disabled} onChange={e=>setQuery(e.target.value)}/></label>
  <p>{t('selectedTagWorks',{count:selected.length})}</p>
  <div className="inline"><button type="button" disabled={disabled||page.loading||!!page.error} onClick={()=>onChange([...new Set([...selected,...rows.map(w=>w.id)])])}>{t('tagSelectPage')}</button><button type="button" disabled={disabled} onClick={()=>onChange([])}>{t('tagSelectNone')}</button></div>
  {membership.error&&<p role="alert">{membership.error}</p>}
  {page.loading&&<p role="status">{t('loading')}</p>}
  {page.error&&<div role="alert"><p>{page.error}</p><button type="button" disabled={disabled} onClick={page.reload}>{t('retry')}</button></div>}
  <div className="custom-tag-works">{rows.map(w=><button type="button" disabled={disabled} key={w.id} aria-pressed={chosen.has(w.id)} onClick={()=>onChange(chosen.has(w.id)?selected.filter(id=>id!==w.id):[...selected,w.id])}>{titleFor(w,titleMode)}{!membership.loading&&!membership.error&&members.has(w.id)?' ✓':''}</button>)}</div>
  {!page.loading&&!page.error&&rows.length===0&&<p>{t('noResults')}</p>}
  <nav className="inline" aria-label={t('collectionPages')}><button type="button" disabled={disabled||page.loading||page.pageIndex===0} onClick={page.previous}>{t('previousPage')}</button><span>{page.pageIndex+1}</span><button type="button" disabled={disabled||page.loading||!!page.error||!page.data?.next} onClick={page.next}>{t('nextPage')}</button></nav>
 </section>;
}
