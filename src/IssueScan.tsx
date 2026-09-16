import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api} from './api';
type Preview={root_id:string;label:string;path:string;exclude:string[];digest:string};
export function IssueScan({issue,onClose,onQueued}:{issue:{id:number;revision:number};onClose:()=>void;onQueued:()=>void}){
 const {t}=useTranslation(),dialog=useRef<HTMLDialogElement>(null),sending=useRef(false);const [preview,setPreview]=useState<Preview|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');
 useEffect(()=>{let active=true;dialog.current?.showModal();setBusy(true);api<Preview>(`/issues/${issue.id}/scan-preview`,{revision:issue.revision}).then(p=>{if(active)setPreview(p);}).catch(e=>{if(active)setError(e instanceof Error?e.message:String(e));}).finally(()=>{if(active)setBusy(false);});return()=>{active=false;};},[issue.id,issue.revision]);
 async function confirm(){if(!preview||sending.current)return;sending.current=true;setBusy(true);setError('');try{await api(`/issues/${issue.id}/scan`,{revision:issue.revision,digest:preview.digest});onQueued();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{sending.current=false;setBusy(false);}}
 return <dialog ref={dialog} className="match-history-dialog" aria-labelledby="issue-scan-title" onCancel={e=>{if(busy)e.preventDefault();else onClose();}}><header><h2 id="issue-scan-title">{t('issueScan')}</h2><button disabled={busy} aria-label={t('close')} onClick={onClose}>×</button></header>{preview&&<><h3>{preview.label}</h3><p className="path">{preview.path}</p><p>{t('issueScanScope')}</p><h4>{t('issueScanExclusions')}</h4>{preview.exclude.length?<ul>{preview.exclude.map((x,i)=><li key={i}>{x}</li>)}</ul>:<p>{t('issueScanNoExclusions')}</p>}</>}{error&&<p role="alert">{error}</p>}{busy&&<p role="status">{t('loading')}</p>}<footer><button disabled={busy} onClick={onClose}>{t('close')}</button><button disabled={busy||!preview} onClick={()=>void confirm()}>{t('issueScanConfirm')}</button></footer></dialog>;
}

