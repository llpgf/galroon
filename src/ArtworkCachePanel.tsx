import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,bytes} from './api';
type Stats={images:number;payload_bytes:number;payload_limit:number;allocated_bytes:number};
export function ArtworkCachePanel(){
 const {t}=useTranslation(),gate=useRef(false);
 const [stats,setStats]=useState<Stats|null>(null),[busy,setBusy]=useState(false),[confirm,setConfirm]=useState(false),[error,setError]=useState(''),[done,setDone]=useState(false);
 async function run(clear=false){if(gate.current)return;gate.current=true;setBusy(true);setError('');setDone(false);try{setStats(await api<Stats>('/artwork/cache',clear?{}:undefined));setConfirm(false);setDone(clear);}catch(e){setError(e instanceof Error?e.message:String(e));}finally{gate.current=false;setBusy(false);}}
 useEffect(()=>{void run();},[]);
 return <section className="section"><h2>{t('artCacheTitle')}</h2><p>{t('artCacheHint')}</p>{stats&&<p>{t('artCacheUsage',{count:stats.images,used:bytes(stats.payload_bytes),limit:bytes(stats.payload_limit),disk:bytes(stats.allocated_bytes)})}</p>}
 {error&&<p role="alert">{error}</p>}{done&&<p role="status">{t('artCacheCleared')}</p>}
 {confirm?<div role="group" aria-label={t('artCacheClear')}><p>{t('artCacheConfirm')}</p><button disabled={busy} onClick={()=>setConfirm(false)}>{t('cancel')}</button><button disabled={busy} onClick={()=>void run(true)}>{t('artCacheConfirmButton')}</button></div>:<div className="inline"><button disabled={busy} onClick={()=>void run()}>{t('artCacheRefresh')}</button><button disabled={busy||!stats||stats.images===0} onClick={()=>{setDone(false);setConfirm(true);}}>{t('artCacheClear')}</button></div>}
 </section>;
}
