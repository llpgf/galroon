import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,SessionChangedError} from './api';
type Watch={enabled:boolean;exclude:string[];revision:number;status:string;message:string;needs_scan:boolean;pending:number;job_id:string|null;job_state:string|null};
export function SourceWatch({rootId,readOnly,onScan}:{rootId:string;readOnly:boolean;onScan:(exclude:string[])=>void}){
 const {t}=useTranslation();const [watch,setWatch]=useState<Watch|null>(null),[editing,setEditing]=useState(false),[exclude,setExclude]=useState(''),[editRevision,setEditRevision]=useState(0),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 useEffect(()=>{let live=true;let pending=false;const refresh=async()=>{if(pending)return;pending=true;try{const value=await api<Watch>(`/roots/${rootId}/watch`);if(live)setWatch(value);}catch(e){if(live&&!(e instanceof SessionChangedError))setError((e as Error).message);}finally{pending=false;}};void refresh();const timer=setInterval(()=>void refresh(),2000);return()=>{live=false;clearInterval(timer);};},[rootId]);
 const save=async(enabled:boolean,exclusions:string[],revision=watch?.revision)=>{if(!watch)return;setBusy(true);setError('');try{setWatch(await api<Watch>(`/roots/${rootId}/watch`,{revision,enabled,exclude:exclusions}));setEditing(false);}catch(e){if(!(e instanceof SessionChangedError))setError((e as Error).message);}finally{setBusy(false);}};
 return <section className="source-watch" aria-label={t('watchTitle')}>
  <h3>{t('watchTitle')}</h3>{watch?<>
   <p><span className="pill">{t(`watch_${watch.status}`)}</span> {t('watchPending',{count:watch.pending})}</p>
   <p>{t('watchHint')}</p>{watch.needs_scan&&<p className="watch-attention">{t(watch.enabled?'watchNeedsScan':'watchNeedsScanDisabled')}</p>}{watch.message&&<p>{watch.message}</p>}
   {watch.job_state&&<p>{t('watchTask',{state:t(watch.job_state)})}</p>}{watch.exclude.length>0&&<p>{t('watchExcluded',{folders:watch.exclude.join(', ')})}</p>}
   {!readOnly&&<div className="inline"><button disabled={busy} onClick={()=>void save(!watch.enabled,watch.exclude)}>{t(watch.enabled?'watchDisable':'watchEnable')}</button><button disabled={busy} onClick={()=>{setExclude(watch.exclude.join('\n'));setEditRevision(watch.revision);setEditing(!editing);}}>{t('watchExclusions')}</button><button disabled={busy} onClick={()=>onScan(watch.exclude)}>{t('watchScan')}</button></div>}
   {editing&&!readOnly&&<div className="watch-editor"><label>{t('watchExclusionLabel')}<textarea value={exclude} onChange={e=>setExclude(e.target.value)} rows={3}/></label><p>{t('watchExclusionHint')}</p><button disabled={busy} onClick={()=>void save(watch.enabled,exclude.split('\n').map(s=>s.trim()).filter(Boolean),editRevision)}>{t('save')}</button></div>}
  </>:<p>{t('loading')}</p>}{error&&<p role="alert" className="error-text">{error}</p>}
 </section>;
}
