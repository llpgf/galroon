import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {bytes} from './api';
import {clearClientArtwork,clientArtworkStats} from './clientArtworkCache';
export function ClientCachePanel(){
 const {t}=useTranslation(),[stats,setStats]=useState(clientArtworkStats),[done,setDone]=useState(false);
 useEffect(()=>{const timer=setInterval(()=>setStats(clientArtworkStats()),2000);return()=>clearInterval(timer);},[]);
 return <section className="section"><h2>{t('clientCacheTitle')}</h2><p>{t('clientCacheHint')}</p><p>{t('clientCacheUsage',{count:stats.images,used:bytes(stats.bytes),limit:bytes(stats.limit)})}</p>
 <button onClick={()=>{clearClientArtwork();setStats(clientArtworkStats());setDone(true);}}>{t('clientCacheClear')}</button>{done&&<p role="status">{t('clientCacheCleared')}</p>}</section>;
}
