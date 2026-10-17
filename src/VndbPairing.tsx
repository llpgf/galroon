import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,type Work} from './api';
type Option={id:string;title:string;released?:string|null;reason?:string;taken?:boolean};
type Found={id:string;title:string;alttitle?:string;released?:string};
/** A card without files (imported from Bangumi) gets its VNDB identity here: the background lookup's candidates, or a search. */
export function VndbPairing({work,onChanged}:{work:Work;onChanged:()=>Promise<void>}){
 const {t}=useTranslation();
 const [state,setState]=useState(''),[options,setOptions]=useState<Option[]>([]),[query,setQuery]=useState(work.original_title||work.title),[busy,setBusy]=useState(false),[error,setError]=useState('');
 useEffect(()=>{let active=true;api<{state:string;candidates:Option[]}>(`/bangumi/vndb/${encodeURIComponent(work.id)}`).then(r=>{if(active){setState(r.state);setOptions(r.candidates);}}).catch(()=>{});return()=>{active=false;};},[work.id]);
 async function pair(id:string){setBusy(true);setError('');try{await api('/bangumi/vndb',{work_id:work.id,revision:work.revision,vndb_id:id});await onChanged();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 async function search(){setBusy(true);setError('');try{const r=await api<{results:Found[]}>('/vndb/search',{query});setState('searched');setOptions(r.results.slice(0,8).map(v=>({id:v.id,title:v.alttitle||v.title,released:v.released})));}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 return <section className="section vndb-pairing"><h2>{t('vndbPairTitle')}</h2><p>{t(state==='review'?'vndbPairReview':state==='pending'?'vndbPairPending':'vndbPairHint')}</p>
  {options.length>0&&<div className="plan-list">{options.map(o=><div className="plan-item" key={o.id}><div><strong>{o.title}</strong><p><a href={`https://vndb.org/${o.id}`} target="_blank" rel="noreferrer">{o.id}</a>{o.released?` · ${o.released}`:''}{o.reason?` · ${t('vndbPairReason_'+o.reason)}`:''}</p>{o.taken&&<p>{t('vndbPairTaken')}</p>}</div><button className="primary" disabled={busy||o.taken} onClick={()=>void pair(o.id)}>{t('vndbPair')}</button></div>)}</div>}
  {state==='searched'&&!options.length&&<p>{t('noResults')}</p>}
  <form className="inline" onSubmit={e=>{e.preventDefault();if(query.trim())void search();}}><input value={query} onChange={e=>setQuery(e.target.value)} aria-label={t('vndbPairSearch')} disabled={busy}/><button disabled={busy||!query.trim()}>{t('vndbPairSearch')}</button></form>
  {error&&<p className="error" role="alert">{error}</p>}
 </section>;
}
