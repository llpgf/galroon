import {useEffect,useRef,useState} from 'react';
import {Search,Clock3,X} from 'lucide-react';
import {useTranslation} from 'react-i18next';
import {readHistory,rememberSearch,writeHistory} from './search';
export function CollectionSearch({query,onChange,storageKey}:{query:string;onChange:(s:string)=>void;storageKey:string}){
 const {t}=useTranslation();const [open,setOpen]=useState(false),[history,setHistory]=useState(()=>readHistory(storageKey));const input=useRef<HTMLInputElement>(null);
 useEffect(()=>{const read=()=>setHistory(readHistory(storageKey));read();window.addEventListener('galroon-search-history',read);window.addEventListener('storage',read);return()=>{window.removeEventListener('galroon-search-history',read);window.removeEventListener('storage',read);};},[storageKey]);
 return <div className="collection-search" onBlur={e=>{if(!e.currentTarget.contains(e.relatedTarget))setOpen(false);}} onKeyDown={e=>{if(e.key==='Escape'){setOpen(false);input.current?.focus();}}}>
  <form role="search" onSubmit={e=>{e.preventDefault();rememberSearch(query,storageKey);setOpen(false);}}><label className="search"><Search size={17}/><input ref={input} aria-label={t('search')} placeholder={t('search')} autoComplete="off" maxLength={200} value={query} onFocus={()=>setOpen(true)} onClick={()=>setOpen(true)} onChange={e=>{onChange(e.target.value);setOpen(true);}} aria-expanded={open&&history.length>0} aria-controls="search-history"/></label>{query&&<button type="button" className="clear-search icon-button" aria-label={t('clearSearch')} onClick={()=>{onChange('');input.current?.focus();}}><X size={15}/></button>}</form>
  {open&&history.length>0&&<div className="search-history" id="search-history"><div className="history-heading"><span>{t('recentSearches')}</span><button className="icon-button" aria-label={t('clearSearchHistory')} onClick={()=>writeHistory(storageKey,[])}><X size={14}/></button></div>{history.map(value=><div className="history-row" key={value}><button className="history-query" onClick={()=>{onChange(value);rememberSearch(value,storageKey);setOpen(false);}}><Clock3 size={14}/><span>{value}</span></button><button className="icon-button" aria-label={t('removeSearch',{query:value})} onClick={()=>writeHistory(storageKey,history.filter(s=>s!==value))}><X size={14}/></button></div>)}</div>}
 </div>;
}
