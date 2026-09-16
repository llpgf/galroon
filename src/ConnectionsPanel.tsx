import {useEffect,useState,useSyncExternalStore} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {useTranslation} from 'react-i18next';
import {selectConnection,updateConnectionEndpoint,subscribeConnection,connectionSnapshot} from './api';
type Saved={selected:string|null;entries:{id:string;name:string;url:string;library_id:string;device_id:string}[]};
export function ConnectionsPanel(){
 const connection=useSyncExternalStore(subscribeConnection,connectionSnapshot);
 const [editing,setEditing]=useState<{id:string;name:string;url:string}|null>(null);
 const {t}=useTranslation();const [saved,setSaved]=useState<Saved|null>(null),[url,setUrl]=useState('http://127.0.0.1:14800'),[code,setCode]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[review,setReview]=useState<{kind:'switch'|'revoke'|'forget';id:string|null;name:string}|null>(null);
 const run=async(work:()=>Promise<void>)=>{setBusy(true);setError('');try{await work();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}};
 useEffect(()=>{invoke<Saved>('list_connections').then(setSaved).catch(e=>setError(String(e)));},[]);
 return <section className="section connections-panel"><h2>{t('savedCores')}</h2><p>{t('pairedCoreScope')}</p>
 {saved&&<div className="saved-connections">{[{id:null,name:t('thisComputerCore'),url:''},...saved.entries].map(entry=><div className="saved-connection" key={entry.id||'local'}>
 <div><strong>{entry.name}</strong>{entry.url&&<small>{entry.url}</small>}</div>
 {saved.selected===entry.id?<span className="pill" role="status">{t('selectedCore')} · {t(connection.phase==='connected'?'connected':'connection_'+connection.phase)}</span>:<button disabled={busy} onClick={()=>setReview({kind:'switch',id:entry.id,name:entry.name})}>{t('openCollection')}</button>}
 {entry.id&&<button disabled={busy} onClick={()=>{setReview(null);setEditing({id:entry.id!,name:entry.name,url:entry.url});}}>{t('changeCoreAddress')}</button>}
 {entry.id&&saved.selected!==entry.id&&<><button disabled={busy} onClick={()=>setReview({kind:'revoke',id:entry.id,name:entry.name})}>{t('revokePairing')}</button><button disabled={busy} onClick={()=>setReview({kind:'forget',id:entry.id,name:entry.name})}>{t('forgetConnection')}</button></>}
 </div>)}</div>}
 {editing&&<form aria-label={t('changeCoreAddress')} onSubmit={e=>{e.preventDefault();void run(async()=>{await updateConnectionEndpoint(editing.id,editing.url);});}}><h3>{editing.name}</h3><p>{t('recoverAddressReview')}</p><label>{t('coreAddress')}<input value={editing.url} disabled={busy} onChange={e=>setEditing({...editing,url:e.target.value})} spellCheck={false}/></label><div className="inline"><button className="primary" disabled={busy||!editing.url.trim()}>{t('verifyCoreAddress')}</button><button type="button" disabled={busy} onClick={()=>setEditing(null)}>{t('cancel')}</button></div></form>}
 {review&&<div className="plan-item" role="group" aria-label={t('reviewConnection')}><strong>{review.name}</strong><p>{t(review.kind==='switch'?'switchCoreReview':review.kind==='revoke'?'revokeCoreReview':'forgetCoreReview')}</p><div className="inline"><button disabled={busy} className="primary" onClick={()=>void run(async()=>{if(review.kind==='switch'){await selectConnection(review.id);}else{setSaved(await invoke<Saved>('remove_connection',{id:review.id,forget:review.kind==='forget'}));setReview(null);}})}>{t('confirmConnection')}</button><button disabled={busy} onClick={()=>setReview(null)}>{t('cancel')}</button></div></div>}
 <form onSubmit={e=>{e.preventDefault();void run(async()=>{setSaved(await invoke<Saved>('pair_connection',{url,code}));setCode('');});}}>
 <h3>{t('addPairedCore')}</h3><label>{t('coreAddress')}<input value={url} onChange={e=>setUrl(e.target.value)} disabled={busy} spellCheck={false}/></label>
 <label>{t('pairingCode')}<input value={code} onChange={e=>setCode(e.target.value)} disabled={busy} maxLength={20} autoComplete="off" spellCheck={false}/></label>
 <button className="primary" disabled={busy||!saved||code.replace(/-/g,'').trim().length!==12}>{t('savePairing')}</button>
 </form>{busy&&<p role="status">{t('loading')}</p>}{error&&<p role="alert" className="error-text">{error}</p>}</section>;
}
