import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import type {Resource} from './api';
import {useResourcePage} from './useResourcePage';
export function ResourceChoice({source,value,onChange,disabled}:{source:Resource;value:Resource|null;onChange:(resource:Resource|null)=>void;disabled:boolean}){
 const {t}=useTranslation(),[query,setQuery]=useState('');
 const page=useResourcePage(query,'all',0,source.root_id,source.id),rows=page.data?.items||[];
 return <section className="resource-choice"><label>{t('resourceChoiceSearch')}<input type="search" value={query} disabled={disabled} onChange={e=>setQuery(e.target.value)}/></label><label>{t('groupDestination')}<select disabled={disabled||page.loading||!!page.error} value={value?.id||''} onChange={e=>onChange(rows.find(r=>r.id===e.target.value)||null)}><option value="">{t('newResourceGroup')}</option>{value&&!rows.some(r=>r.id===value.id)&&<option value={value.id}>{value.title}</option>}{rows.map(r=><option key={r.id} value={r.id}>{r.title} · {r.files} {t('files')}</option>)}</select></label>{page.loading&&<p role="status">{t('loading')}</p>}{page.error&&<div role="alert"><p>{page.error}</p><button disabled={disabled} onClick={page.reload}>{t('retry')}</button></div>}<div className="inline"><button disabled={disabled||page.loading||page.pageIndex===0} onClick={page.previous}>{t('previousPage')}</button><span>{page.pageIndex+1}</span><button disabled={disabled||page.loading||!page.data?.next} onClick={page.next}>{t('nextPage')}</button></div></section>;
}
