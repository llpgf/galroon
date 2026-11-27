import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {Archive,FolderPlus,RefreshCw,ArrowLeft,AlertTriangle,Check} from 'lucide-react';
import {api,ApiRequestError,bytes,coreHealth} from './api';
import {BrandMark} from './BrandMark';
export type RestoreCandidate={
 path:string;
 name:string;
 created:number;
 version:number;
 mode:'full'|'portable';
 origin:'scheduled'|'manual';
 bytes:number;
 sha256:string;
 schema:number;
 has_artwork:boolean;
 artwork_bytes:number;
 library:{id:string;name:string;revision:number};
 spki_fingerprint:string|null;
 core_version:string|null;
};
export type RestoreCandidatesResponse={
 candidates:RestoreCandidate[];
 searched:string[];
};
export type RestoreInspectResponse={
 path:string;
 manifest:Record<string,unknown>;
 mode:'full'|'portable';
 origin:'scheduled'|'manual';
 sha256:string;
 schema:number;
 works:number;
 resources:number;
 files:number;
 roots:number;
 spki_fingerprint:string|null;
 device_id:string|null;
 requires_original_offline:boolean;
 network_address:string|null;
 network_address_local:boolean|null;
};
export type RestoreStartResponse={
 status:string;
 mode:'full'|'portable';
 collection:string;
 relaunch:boolean;
 lan_disabled:boolean;
 warning:string|null;
 replaced_identity:string|null;
 requires_new_session:boolean;
};
type Step='choose'|'create'|'restore'|'restarting'|'restarted';
export function FirstRun({
 initialCandidates,
 onComplete,
 onSkip,
 onReconnect,
 pollIntervalMs=1000,
 maxPollAttempts=60,
}:{
 initialCandidates?:RestoreCandidatesResponse;
 onComplete:()=>Promise<void>|void;
 onSkip?:()=>void;
 onReconnect?:()=>void;
 pollIntervalMs?:number;
 maxPollAttempts?:number;
}){
 const {t}=useTranslation();
 const [fresh,setFresh]=useState<boolean|null>(initialCandidates?true:null);
 const [step,setStep]=useState<Step>('choose');
 const [candidates,setCandidates]=useState<RestoreCandidate[]>(initialCandidates?.candidates||[]);
 const [searched,setSearched]=useState<string[]>(initialCandidates?.searched||[]);
 const [password,setPassword]=useState('');
 const [confirm,setConfirm]=useState('');
 const [manualPath,setManualPath]=useState('');
 const [inspect,setInspect]=useState<RestoreInspectResponse|null>(null);
 const [originalOffline,setOriginalOffline]=useState(false);
 const [restoreResult,setRestoreResult]=useState<RestoreStartResponse|null>(null);
 const [pollTimedOut,setPollTimedOut]=useState(false);
 const [pollRound,setPollRound]=useState(0);
 const [busy,setBusy]=useState(false);
 const [error,setError]=useState('');
 const loadCandidates=async()=>{
  setBusy(true);setError('');
  try{
   const data=await api<RestoreCandidatesResponse>('/setup/restore/candidates');
   setFresh(true);
   setCandidates(data.candidates||[]);
   setSearched(data.searched||[]);
  }catch(e){
   if(e instanceof ApiRequestError&&(e.status===409||e.code==='core_not_fresh')){
    setFresh(false);
    setStep('create');
   }else{
    setFresh(true);
    setError(e instanceof Error?e.message:String(e));
   }
  }finally{
   setBusy(false);
  }
 };
 useEffect(()=>{
  if(initialCandidates!==undefined)return;
  void loadCandidates();
 },[]);
 useEffect(()=>{
  if(step!=='restarting')return;
  let cancelled=false,attempts=0,sawDown=false,timer:ReturnType<typeof setTimeout>|undefined;
  const controller=new AbortController();
  setPollTimedOut(false);
  const check=async()=>{
   if(cancelled)return;
   attempts++;
   const ok=await coreHealth(controller.signal);
   if(cancelled)return;
   if(!ok){
    sawDown=true;
   }else if(sawDown||attempts>=2){
    setStep('restarted');
    return;
   }
   if(attempts>=maxPollAttempts){
    setPollTimedOut(true);
    return;
   }
   timer=setTimeout(()=>{void check();},pollIntervalMs);
  };
  timer=setTimeout(()=>{void check();},pollIntervalMs);
  return()=>{cancelled=true;controller.abort();if(timer!==undefined)clearTimeout(timer);};
 },[step,pollRound,pollIntervalMs,maxPollAttempts]);
 const act=async(fn:()=>Promise<void>)=>{
  setBusy(true);setError('');
  try{await fn();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}
 };
 const inspectPath=async(path:string)=>{
  const trimmed=path.trim();
  if(!trimmed)return;
  setManualPath(trimmed);
  setInspect(null);
  setOriginalOffline(false);
  await act(async()=>{
   const result=await api<RestoreInspectResponse>('/setup/restore/inspect',{path:trimmed});
   setInspect(result);
  });
 };
 const needsOfflineCheck=inspect!==null&&(inspect.mode==='full'||inspect.requires_original_offline);
 const canRestore=inspect!==null&&!busy&&(!needsOfflineCheck||originalOffline);
 return <div className="connect first-run">
  <div className="wordmark"><BrandMark/>galroon</div>
  {fresh===null&&<p role="status">{t('loading')}</p>}

  {fresh!==null&&step==='choose'&&<>
   <h1>{t('firstRunTitle')}</h1>
   <p>{t('firstRunHint')}</p>
   <div className="plan-item">
    <h3><FolderPlus size={18}/> {t('firstRunCreateTitle')}</h3>
    <p>{t('firstRunCreateHint')}</p>
    <button className="primary" disabled={busy} onClick={()=>{setError('');setStep('create');}}>{t('firstRunCreateAction')}</button>
   </div>
   <div className="plan-item">
    <h3><Archive size={18}/> {t('firstRunRestoreTitle')}</h3>
    <p>{t('firstRunRestoreHint')}</p>
    <button disabled={busy} onClick={()=>{setError('');setStep('restore');}}>{t('firstRunRestoreAction')}</button>
   </div>
  </>}

  {fresh!==null&&step==='create'&&<>
   {fresh&&<button type="button" disabled={busy} onClick={()=>{setError('');setStep('choose');}}><ArrowLeft size={15}/> {t('back')}</button>}
   <h1>{t('setupOwner')}</h1>
   {!fresh&&<p role="status">{t('firstRunNotFreshNote')}</p>}
   <p>{t('ownerPasswordHint')}</p>
   <form aria-label={t('setupOwner')} onSubmit={e=>{
    e.preventDefault();
    void act(async()=>{
     await api('/access/setup',{password});
     setPassword('');setConfirm('');
     await onComplete();
    });
   }}>
    <label>{t('ownerPassword')}<input type="password" autoComplete="new-password" value={password} disabled={busy} onChange={e=>setPassword(e.target.value)}/></label>
    <label>{t('confirmOwnerPassword')}<input type="password" autoComplete="new-password" value={confirm} disabled={busy} onChange={e=>setConfirm(e.target.value)}/></label>
    <div className="inline">
     <button className="primary" disabled={busy||Array.from(password).length<12||password!==confirm}>{t('confirmOwnerSetup')}</button>
     {onSkip&&<button type="button" disabled={busy} onClick={onSkip}>{t('firstRunSkipOwner')}</button>}
    </div>
   </form>
  </>}

  {fresh!==null&&step==='restore'&&<>
   <button type="button" disabled={busy} onClick={()=>{setError('');setStep('choose');}}><ArrowLeft size={15}/> {t('back')}</button>
   <h1>{t('firstRunRestoreTitle')}</h1>
   <p>{t('firstRunRestoreHint')}</p>
   <div className="inline">
    {searched.length>0&&<small className="muted">{t('firstRunSearchedDirs',{dirs:searched.join(', ')})}</small>}
    <button type="button" disabled={busy} onClick={()=>void loadCandidates()}><RefreshCw size={14}/> {t('firstRunRefreshCandidates')}</button>
   </div>
   {candidates.length===0&&<p className="muted">{t('firstRunNoCandidates')}</p>}
   {candidates.map(c=><article className="plan-item" key={c.path}>
    <div className="inline">
     <strong>{c.library?.name||c.name}</strong>
     <span className="pill">{t(c.mode==='full'?'firstRunModeFull':'firstRunModePortable')}</span>
     <span className="pill">{t(c.origin==='scheduled'?'firstRunOriginScheduled':'firstRunOriginManual')}</span>
    </div>
    <p>{new Date(c.created*1000).toLocaleString()} · {bytes(c.bytes+(c.artwork_bytes||0))} · Schema {c.schema}</p>
    <p className="path">{c.path}</p>
    {c.spki_fingerprint&&<small>SPKI: {c.spki_fingerprint}</small>}
    <div className="inline">
     <button type="button" disabled={busy} onClick={()=>void inspectPath(c.path)}>{t('firstRunSelectCandidate')}</button>
    </div>
   </article>)}

   <form aria-label={t('inspectBackup')} onSubmit={e=>{e.preventDefault();void inspectPath(manualPath);}}>
    <label>{t('firstRunBackupPath')}<input value={manualPath} disabled={busy} placeholder="/backups/Galroon-backup-..." spellCheck={false} onChange={e=>{setManualPath(e.target.value);setInspect(null);setOriginalOffline(false);}}/></label>
    <p className="muted">{t('corePathHint')}</p>
    <button disabled={busy||!manualPath.trim()}>{t('inspectBackup')}</button>
   </form>

   {inspect&&<div className="plan-item" role="region" aria-label={t('firstRunInspectTitle')}>
    <h3>{t('firstRunInspectTitle')}</h3>
    <div className="inline">
     <span className="pill">{t(inspect.mode==='full'?'firstRunModeFull':'firstRunModePortable')}</span>
     <span className="pill">{t(inspect.origin==='scheduled'?'firstRunOriginScheduled':'firstRunOriginManual')}</span>
     <span className="pill">Schema {inspect.schema}</span>
    </div>
    <p>{t('firstRunInspectCounts',{works:inspect.works,resources:inspect.resources,files:inspect.files,roots:inspect.roots})}</p>
    <p className="path">{inspect.path}</p>
    {inspect.spki_fingerprint&&<p><span>{t('lanSpkiFingerprint')}: </span><span className="path">{inspect.spki_fingerprint}</span></p>}
    {inspect.network_address&&<p>{t('firstRunNetworkAddress',{address:inspect.network_address})}</p>}
    {inspect.network_address&&inspect.network_address_local===false&&<p className="error-text" role="alert"><AlertTriangle size={14}/> {t('firstRunNetworkNotLocal',{address:inspect.network_address})}</p>}
    {needsOfflineCheck&&<>
     <p className="error-text"><AlertTriangle size={14}/> {t('firstRunFullModeWarning')}</p>
     <label className="checkbox-label"><input type="checkbox" checked={originalOffline} disabled={busy} onChange={e=>setOriginalOffline(e.target.checked)}/>{t('firstRunOriginalOfflineConfirm')}</label>
    </>}
    <button className="primary" disabled={!canRestore} onClick={()=>void act(async()=>{
     const started=await api<RestoreStartResponse>('/setup/restore',{path:inspect.path,sha256:inspect.sha256,original_offline:originalOffline});
     setRestoreResult(started);
     setStep('restarting');
    })}>{t('firstRunStartRestore')}</button>
   </div>}
  </>}

  {step==='restarting'&&<div className="plan-item" role="status">
   <h2><RefreshCw size={18} className="spin"/> {t('firstRunRestartingTitle')}</h2>
   <p>{t('firstRunRestartingHint')}</p>
   {restoreResult?.lan_disabled&&<div className="plan-item" role="alert"><strong>{t('firstRunLanDisabledTitle')}</strong><p>{restoreResult.warning||t('firstRunLanDisabledWarning')}</p></div>}
   {pollTimedOut&&<>
    <p className="error-text" role="alert">{t('firstRunRestartTimeout')}</p>
    <div className="inline">
     <button type="button" onClick={()=>setPollRound(n=>n+1)}>{t('firstRunCheckHealth')}</button>
     <button type="button" className="primary" onClick={()=>onReconnect?onReconnect():window.location.reload()}>{t('firstRunReload')}</button>
    </div>
   </>}
  </div>}

  {step==='restarted'&&<div className="plan-item" role="status">
   <h2><Check size={18}/> {t('firstRunRestartedTitle')}</h2>
   {restoreResult?.lan_disabled&&<div className="plan-item" role="alert"><strong>{t('firstRunLanDisabledTitle')}</strong><p>{restoreResult.warning||t('firstRunLanDisabledWarning')}</p></div>}
   <p>{t(restoreResult?.mode==='full'?'firstRunReconnectFull':'firstRunReconnectPortable')}</p>
   <button type="button" className="primary" onClick={()=>onReconnect?onReconnect():window.location.reload()}>{t('firstRunReload')}</button>
  </div>}

  {busy&&<p role="status">{t('loading')}</p>}
  {error&&<p className="error-text" role="alert">{error}</p>}
 </div>;
}
