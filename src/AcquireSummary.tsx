import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,bytes,type Resource} from './api';
type File={id:string;path:string;relative:string;size:number;archive_entry:boolean};
type Summary={source_folder:string;files:File[];bytes:number;volume_groups:{entry:string;members:string[]}[];manifest_digest:string};
export type TransferSelection={resourceId:string;sourceFolder:string;file_ids:string[];manifest_digest:string;hasArchive:boolean};
export function AcquireSummary({resource,onSelection,disabled=false}:{resource:Resource;disabled?:boolean;onSelection:(selection:TransferSelection|null)=>void}){
 const {t}=useTranslation();const [summary,setSummary]=useState<Summary|null>(null),[selected,setSelected]=useState<string[]>([]),[error,setError]=useState(''),[revision,setRevision]=useState(0);
 const update=(s:Summary,ids:string[])=>{setSelected(ids);onSelection({resourceId:resource.id,sourceFolder:s.source_folder,file_ids:ids,manifest_digest:s.manifest_digest,hasArchive:s.files.some(f=>ids.includes(f.id)&&f.archive_entry)});};
 useEffect(()=>{let current=true;onSelection(null);setSummary(null);setSelected([]);setError('');api<Summary>(`/resources/${resource.id}/acquire-preview`).then(s=>{if(current){setSummary(s);update(s,s.files.map(f=>f.id));}}).catch(e=>{if(current)setError(e.message);});return()=>{current=false;};},[resource.id,revision]);
 const toggle=(file:File,checked:boolean)=>{if(!summary)return;const group=summary.volume_groups.find(g=>g.members.includes(file.path));const ids=summary.files.filter(f=>group?group.members.includes(f.path):f.id===file.id).map(f=>f.id);update(summary,checked?[...new Set([...selected,...ids])]:selected.filter(id=>!ids.includes(id)));};
 return <section className="acquire-summary">
  {summary?<>
   <p aria-live="polite">{t('transferSelection',{count:selected.length,total:summary.files.length,size:bytes(summary.files.filter(f=>selected.includes(f.id)).reduce((n,f)=>n+f.size,0))})}</p>
   <p>{t('transferSelectionHint')}</p>
   {summary.volume_groups.length>0&&<p>{t('volumeVerificationHint',{count:summary.volume_groups.length})}</p>}
   <div className="transfer-selection-actions"><button disabled={disabled} onClick={()=>update(summary,summary.files.map(f=>f.id))}>{t('selectAllFiles')}</button><button disabled={disabled} onClick={()=>update(summary,[])}>{t('clearFileSelection')}</button></div>
   <details open><summary>{t('reviewTransferFiles')}</summary><div className="transfer-file-list">{summary.files.map(f=>{const group=summary.volume_groups.find(g=>g.members.includes(f.path));return <label className="transfer-file" key={f.id}><input type="checkbox" disabled={disabled} checked={selected.includes(f.id)} onChange={e=>toggle(f,e.target.checked)}/><span><span className="path" title={f.path}>{f.relative}</span><small>{bytes(f.size)}{group&&<> · {t('transferVolume',{count:group.members.length})}</>}</small></span></label>;})}</div></details>
  </>:!error&&<p role="status">{t('loading')}</p>}
  {error&&<p className="error-text" role="alert">{error}</p>}
  <button onClick={()=>setRevision(n=>n+1)} disabled={disabled||!summary&&!error}>{t('reloadFilePreview')}</button>
 </section>;
}
