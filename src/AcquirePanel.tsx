import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {FolderOpen} from 'lucide-react';
import {AcquireSummary,type TransferSelection} from './AcquireSummary';
import {collectionStorageKey,api,folder,type Resource} from './api';
const destinationKey=()=>collectionStorageKey('acquire.destination');
function savedDestination(){try{return localStorage.getItem(destinationKey())||'';}catch{return '';}}
export function AcquirePanel({resource,onStarted,deviceName}:{resource:Resource;onStarted:()=>void;deviceName?:string}){
 const {t}=useTranslation();const [selection,setSelection]=useState<TransferSelection|null>(null);
 const [saved,setSaved]=useState(savedDestination),[destination,setDestination]=useState(savedDestination),[reuse,setReuse]=useState(false),[extract,setExtract]=useState(false),[password,setPassword]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
 const changeDefault=(value:string)=>{try{if(value)localStorage.setItem(destinationKey(),value);else localStorage.removeItem(destinationKey());setSaved(value);setNotice(t(value?'destinationSaved':'destinationCleared'));setError('');}catch{setError(t('destinationSaveFailed'));}};
 const target=reuse?selection?.sourceFolder||'':destination.trim();
 const start=async()=>{if(!selection||!target||!selection.file_ids.length)return;setBusy(true);setError('');try{await api('/acquire',{resource_id:resource.id,destination:target,extract:!reuse&&extract&&selection.hasArchive,file_ids:selection.file_ids,manifest_digest:selection.manifest_digest,password:password||undefined});onStarted();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setPassword('');setBusy(false);}};
 return <>
  <div className="eyebrow">{t('localAcquisition')}</div><h2>{t('acquireTitle')}</h2><p>{t('acquireHint')}</p><p>{t('operationOnComputer',{device:deviceName||t('unknown')})}</p><p>{resource.title}</p>
  <label>{t('acquisitionAction')}<select value={reuse?'reuse':'copy'} disabled={busy} onChange={e=>{setReuse(e.target.value==='reuse');setError('');}}><option value="copy">{t('copyToFolder')}</option><option value="reuse">{t('useExistingFiles')}</option></select></label>
  <AcquireSummary resource={resource} onSelection={setSelection} disabled={busy}/>
  {reuse?<><p>{t('reuseExistingHint')}</p><label>{t('existingFolder')}<input readOnly value={selection?.sourceFolder||''} title={selection?.sourceFolder||''}/></label></>:<>
   <label>{t('destination')}<input value={destination} disabled={busy} onChange={e=>{setDestination(e.target.value);setNotice('');}}/></label>
   <div className="destination-actions"><button disabled={busy} onClick={()=>{void folder().then(p=>{if(p){setDestination(p);setNotice('');}}).catch(e=>setError(String(e)));}}><FolderOpen size={16}/>{t('browse')}</button><button disabled={busy||!destination.trim()} onClick={()=>changeDefault(destination.trim())}>{t('saveDefaultDestination')}</button>{saved&&<><button disabled={busy} onClick={()=>setDestination(saved)}>{t('useDefaultDestination')}</button><button disabled={busy} onClick={()=>changeDefault('')}>{t('clearDefaultDestination')}</button></>}</div>
   <p className="muted">{t('destinationDefaultHint')}{saved&&<> <span className="path">{saved}</span></>}</p><p>{t('sameDestinationHint')}</p>
   <label className="checkbox-label"><input type="checkbox" checked={extract&&selection?.hasArchive===true} disabled={busy||!selection?.hasArchive} onChange={e=>setExtract(e.target.checked)}/>{t('extract')}</label>
  </>}
  {selection?.hasArchive&&<label>{t('password')}<input type="password" disabled={busy} value={password} onChange={e=>setPassword(e.target.value)} autoComplete="off"/><small>{t('passwordHint')}</small></label>}
  {notice&&<p role="status">{notice}</p>}{error&&<p className="error-text" role="alert">{error}</p>}
  <footer><button className="primary" disabled={busy||!target||!selection||selection.resourceId!==resource.id||!selection.file_ids.length} onClick={()=>void start()}>{t(reuse?'confirmExistingFiles':'startTransfer')}</button></footer>
 </>;
}
