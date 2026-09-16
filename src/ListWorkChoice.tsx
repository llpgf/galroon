import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import type {Work} from './api';
import {emptyFilters} from './library';
import {useCollectionPage} from './useCollectionPage';

/** Retain the selected identity and title while browsing other result pages. */
export function ListWorkChoice({value,onChange,disabled,excludeId}:{value:Work|null;onChange:(work:Work|null)=>void;disabled:boolean;excludeId?:string}){
 const {t,i18n}=useTranslation(),[query,setQuery]=useState('');
 const page=useCollectionPage({filters:emptyFilters,query,status:'all',sort:'title',titleMode:'display',locale:i18n.language},0);
 const rows=(page.data?.items||[]).filter(w=>w.id!==excludeId);
 return <section className="list-work-choice"><label>{t('findWorks')}<input type="search" disabled={disabled} value={query} onChange={e=>setQuery(e.target.value)}/></label><label>{t('listWork')}<select disabled={disabled||page.loading||!!page.error} value={value?.id||''} onChange={e=>onChange(rows.find(w=>w.id===e.target.value)||null)}><option value="">{t('listChooseWork')}</option>{value&&!rows.some(w=>w.id===value.id)&&<option value={value.id}>{value.title}</option>}{rows.map(w=><option key={w.id} value={w.id}>{w.title}</option>)}</select></label>{page.loading&&<p role="status">{t('loading')}</p>}{page.error&&<div role="alert"><p>{page.error}</p><button type="button" disabled={disabled} onClick={page.reload}>{t('retry')}</button></div>}<div className="inline"><button type="button" disabled={disabled||page.loading||page.pageIndex===0} onClick={page.previous}>{t('previousPage')}</button><span>{page.pageIndex+1}</span><button type="button" disabled={disabled||page.loading||!page.data?.next} onClick={page.next}>{t('nextPage')}</button></div></section>;
}
