import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {useTranslation} from 'react-i18next';
import {Archive,FolderOpen,Trash2} from 'lucide-react';
import {api,bytes,deleteApi,folder,isLocalDesktop,restoreCore} from './api';
import {BackupFullWarning,BackupSchedule,type BackupMode,type BackupOrigin,type BackupScheduleState} from './BackupSchedule';

export type Manifest={
 created:number;
 bytes:number;
 sha256:string;
 version:number;
 mode?:BackupMode;
 origin?:BackupOrigin;
 library?:{id:string;name:string};
 schema?:number;
 verification?:string;
 artwork?:{images:number;profiles:number;bytes:number};
};

export type BackupRecord={
 id:string;
 path:string;
 created:number;
 origin?:BackupOrigin;
 mode?:BackupMode;
 deleted?:boolean;
 manifest:Manifest;
};

export interface BackupPanelProps{
 onRestored:()=>Promise<void>;
 allowRestore?:boolean;
 isOwner?:boolean;
 initialHistory?:BackupRecord[];
 initialSchedule?:BackupScheduleState|null;
}

export function BackupPanel({onRestored,allowRestore=true,isOwner,initialHistory,initialSchedule=null}:BackupPanelProps){
 const {t}=useTranslation();
 const owner=isOwner??allowRestore;
 const canBrowseLocal=isLocalDesktop();
 const [destination,setDestination]=useState('');
 const [manualMode,setManualMode]=useState<BackupMode>('portable');
 const [source,setSource]=useState('');
 const [preview,setPreview]=useState<Manifest|null>(null);
 const [message,setMessage]=useState('');
 const [error,setError]=useState('');
 const [busy,setBusy]=useState(false);
 const [history,setHistory]=useState<BackupRecord[]>(initialHistory??[]);
 const [historyError,setHistoryError]=useState('');
 const [scheduleRefresh,setScheduleRefresh]=useState(0);

 const loadHistory=async()=>{
  try{setHistory(await api<BackupRecord[]>('/backups'));setHistoryError('');}
  catch(e){setHistoryError(e instanceof Error?e.message:String(e));}
 };
 useEffect(()=>{void loadHistory();},[]);

 const [recovery,setRecovery]=useState<{path:string;created:number;library?:{name:string}}[]>([]);
 const [recoveryError,setRecoveryError]=useState('');
 const loadRecovery=async()=>{
  if(!canBrowseLocal||!allowRestore)return;
  try{setRecovery(await invoke('restore_history'));setRecoveryError('');}
  catch(e){setRecoveryError(String(e));}
 };
 useEffect(()=>{void loadRecovery();},[allowRestore,canBrowseLocal]);

 const act=async(work:()=>Promise<void>)=>{
  setBusy(true);setError('');setMessage('');
  try{await work();}
  catch(e){setError(e instanceof Error?e.message:String(e));}
  finally{setBusy(false);}
 };

 return <section className="section backup-panel" style={{minWidth:0,maxWidth:'100%'}}>
  <h2><Archive size={20}/> {t('backupTitle')}</h2>
  <p>{t('backupHint')}</p>

  <BackupSchedule isOwner={owner} refreshToken={scheduleRefresh} initialSchedule={initialSchedule} onChanged={loadHistory}/>

  <h3>{t('backupManualTitle')}</h3>
  <p id="backup-destination-hint">{t(canBrowseLocal?'backupDestinationHint':'backupDestinationCoreHint')}</p>
  <label>{t('backupDestination')}<input style={{width:'100%',minWidth:0,boxSizing:'border-box'}} aria-describedby="backup-destination-hint" value={destination} onChange={e=>setDestination(e.target.value)}/></label>
  <label style={{marginTop:10}}>{t('backupScheduleMode')}
   <select style={{maxWidth:'100%'}} value={manualMode} disabled={busy} onChange={e=>{
    const next=e.target.value as BackupMode;
    if(next==='full'&&!owner)return;
    setManualMode(next);
   }}>
    <option value="portable">{t('backupMode_portable')}</option>
    <option value="full" disabled={!owner}>{t('backupMode_full')}</option>
   </select>
  </label>
  {!owner&&<p className="muted" style={{margin:'6px 0 0',fontSize:12.5}}>{t('backupFullOwnerOnly')}</p>}
  {manualMode==='full'&&<BackupFullWarning/>}
  <div className="inline" style={{marginTop:12}}>
   {canBrowseLocal&&<button type="button" disabled={busy} onClick={()=>void act(async()=>{const path=await folder();if(path)setDestination(path);})}><FolderOpen size={16}/>{t('browse')}</button>}
   <button type="button" disabled={busy||!destination.trim()||(!owner&&manualMode==='full')} onClick={()=>void act(async()=>{
    const result=await api<{path:string}>('/backups',{destination:destination.trim(),mode:manualMode});
    setMessage(t('backupSaved',{path:result.path}));
    await loadHistory();
    setScheduleRefresh(n=>n+1);
   })}>{t('exportBackup')}</button>
  </div>

  <details className="backup-history">
   <summary>{t('recentBackups')}</summary>
   <p>{t('backupHistoryScope')}</p>
   <button type="button" disabled={busy} onClick={()=>void loadHistory()}>{t('refreshBackupHistory')}</button>
   {historyError&&<p role="alert">{historyError}</p>}
   {history.map(item=>{
    const origin:BackupOrigin=item.origin??item.manifest.origin??'manual';
    const mode:BackupMode=item.mode??item.manifest.mode??'portable';
    const deleted=item.deleted===true;
    return <article className="plan-item" key={item.id}>
     <div className="inline" style={{gap:6,marginBottom:6}}>
      <strong>{item.manifest.library?.name||t('backupTitle')}</strong>
      <span className={`pill ${origin==='scheduled'?'running':''}`}>{t(`backupOrigin_${origin}`)}</span>
      <span className={`pill ${mode==='full'?'failed':'completed'}`}>{t(`backupMode_${mode}`)}</span>
      {deleted&&<span className="pill paused">{t('backupDeleted')}</span>}
     </div>
     <p>{new Date(item.created*1000).toLocaleString()} · {bytes(item.manifest.bytes+(item.manifest.artwork?.bytes||0))}</p>
     <p className="path">{item.path}</p>
     {item.manifest.artwork&&<p>{t('backupArtwork',{count:item.manifest.artwork.images,profiles:item.manifest.artwork.profiles,used:bytes(item.manifest.artwork.bytes)})}</p>}
     <p>{t('backupIntegrityAtCreation')}</p>
     <div className="inline">
      {canBrowseLocal&&allowRestore&&!deleted&&<button type="button" disabled={busy} onClick={()=>{setSource(item.path);setPreview(null);}}>{t('chooseBackup')}</button>}
      {origin==='manual'&&!deleted&&<button type="button" disabled={busy} onClick={()=>void act(async()=>{
       await deleteApi<{deleted:boolean}>(`/backups/${encodeURIComponent(item.id)}`);
       setMessage(t('backupDeletedDone'));
       await loadHistory();
       setScheduleRefresh(n=>n+1);
      })}><Trash2 size={15}/>{t('backupDelete')}</button>}
     </div>
    </article>;
   })}
  </details>

  {canBrowseLocal&&allowRestore&&<>
   <h3>{t('restoreBackup')}</h3>
   <details>
    <summary>{t('previousCollections')}</summary>
    <p>{t('previousCollectionsHint')}</p>
    <button type="button" disabled={busy} onClick={()=>void loadRecovery()}>{t('refreshBackupHistory')}</button>
    {recoveryError&&<p role="alert">{recoveryError}</p>}
    {recovery.map(item=><article className="plan-item" key={item.path}>
     <strong>{item.library?.name||t('backupTitle')}</strong>
     <p>{new Date(item.created*1000).toLocaleString()}</p>
     <button type="button" disabled={busy} onClick={()=>{setSource(item.path);setPreview(null);}}>{t('chooseBackup')}</button>
    </article>)}
   </details>
   <label>{t('backupFolder')}<input style={{width:'100%',minWidth:0,boxSizing:'border-box'}} value={source} onChange={e=>{setSource(e.target.value);setPreview(null);}}/></label>
   <div className="inline" style={{marginTop:8}}>
    <button type="button" disabled={busy} onClick={()=>void act(async()=>{const path=await folder();if(path){setSource(path);setPreview(null);}})}><FolderOpen size={16}/>{t('browse')}</button>
    <button type="button" disabled={busy||!source} onClick={()=>void act(async()=>setPreview(await api<Manifest>('/backups/inspect',{path:source})))}>{t('inspectBackup')}</button>
   </div>
   {preview&&<div className="plan-item">
    <p>{t('backupCreated')}: {new Date(preview.created*1000).toLocaleString()}</p>
    <p>{bytes(preview.bytes+(preview.artwork?.bytes||0))}</p>
    {preview.artwork&&<p>{t('backupArtwork',{count:preview.artwork.images,profiles:preview.artwork.profiles,used:bytes(preview.artwork.bytes)})}</p>}
    {preview.library&&<p>{preview.library.name} · {preview.library.id}</p>}
    {preview.schema&&<p>Schema {preview.schema}</p>}
    <p>{t('backupIntegrityAtCreation')}</p>
    <p>{t('restoreHint')}</p>
    <button type="button" className="primary" disabled={busy} onClick={()=>void act(async()=>{
     await restoreCore(source,preview.sha256);
     await onRestored();
     await loadRecovery();
     await loadHistory();
     setPreview(null);
     setMessage(t('restoreComplete'));
    })}>{t('confirmRestore')}</button>
   </div>}
  </>}

  {busy&&<p role="status">{t('loading')}</p>}
  {message&&<p className="path" role="status">{message}</p>}
  {error&&<p className="error-text" role="alert">{error}</p>}
 </section>;
}
