import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,bytes} from './api';
type Job={id:string;state:string;processed:number;discovered:number;errors:number;message:string};
type Status={images:number;bytes:number;profiles:number;job:Job|null;automatic?:{pending:number;errors:number;retry_at:number|null}};
export function OfflineArtworkPanel(){
 const {t}=useTranslation(),gate=useRef(false);
 const [status,setStatus]=useState<Status|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
 useEffect(()=>{let active=true;const load=async()=>{try{const value=await api<Status>('/artwork/offline');if(active){setStatus(value);setError('');}}catch(e){if(active)setError(String(e));}};void load();const timer=setInterval(()=>void load(),2000);return()=>{active=false;clearInterval(timer);};},[]);
 const act=async(path:string)=>{if(gate.current)return;gate.current=true;setBusy(true);setError('');try{await api(path,{});setStatus(await api<Status>('/artwork/offline'));}catch(e){setError(String(e));}finally{gate.current=false;setBusy(false);}};
 const job=status?.job,active=job&&['queued','running','pausing','cancelling'].includes(job.state);
 return <section className="section"><h2>{t('offlineArtworkTitle')}</h2><p>{t('offlineArtworkHint')}</p>
 {status&&<p>{t('offlineArtworkUsage',{count:status.images,used:bytes(status.bytes),profiles:status.profiles})}</p>}
 {status?.automatic&&<p role="status">{t('offlineArtworkAutomatic',{pending:status.automatic.pending,errors:status.automatic.errors})}</p>}
 {job&&<div role="status"><p>{t('offlineArtworkProgress',{state:t(job.state),processed:job.processed,total:job.discovered,errors:job.errors})}</p>{job.errors>0&&<p>{t('offlineArtworkRetryHint')}</p>}{job.message&&<p>{job.message}</p>}</div>}
 <div className="inline"><button disabled={busy||!!active||job?.state==='paused'} onClick={()=>void act('/artwork/offline')}>{t('offlineArtworkDownload')}</button>
 {job?.state==='running'&&<button disabled={busy} onClick={()=>void act(`/jobs/${job.id}/pause`)}>{t('pause')}</button>}
 {job&&['paused','interrupted','failed','cancelled'].includes(job.state)&&<button disabled={busy} onClick={()=>void act(`/jobs/${job.id}/resume`)}>{t('resume')}</button>}</div>
 {error&&<p role="alert">{error}</p>}</section>;
}
