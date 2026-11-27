import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ExternalLink} from 'lucide-react';
import {api,type Work} from './api';
export function VndbWork({work,onChanged,onHidden}:{work:Work;onChanged:()=>Promise<void>;onHidden:()=>void}){
 const {t}=useTranslation();const [subject,setSubject]=useState(work.vndb_id||''),[busy,setBusy]=useState(false),[error,setError]=useState('');
 async function run(path:string,body:object,hidden=false){setBusy(true);setError('');try{await api(path,body);await onChanged();if(hidden)onHidden();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 return <section className="sync-card">
  <div className="sync-card-head"><h2>{t('vnsTitle')}</h2>{work.resources===0&&<span className="pill">{t('availability_no_resources')}</span>}</div>
  {work.vndb_sync_id?<div className="sync-card-body">
   <p><a href={`https://vndb.org/v${work.vndb_sync_id}`} target="_blank" rel="noreferrer">VNDB v{work.vndb_sync_id} <ExternalLink size={12}/></a> · {t(work.vndb_sync_hidden?'vnsKind_hidden':'vnsLinked')}</p>
   <div className="inline"><button disabled={busy} onClick={()=>void run('/vndb_sync/link',{work_id:work.id,revision:work.revision,subject:''})}>{t('vnsUnlink')}</button>{work.resources===0&&<button disabled={busy} onClick={()=>void run('/vndb_sync/hide',{work_id:work.id,hidden:!work.vndb_sync_hidden},!work.vndb_sync_hidden)}>{t(work.vndb_sync_hidden?'vnsRestore':'vnsHide')}</button>}</div>
  </div>:<form className="sync-form" onSubmit={e=>{e.preventDefault();void run('/vndb_sync/link',{work_id:work.id,revision:work.revision,subject});}}>
   <p className="muted">{t('vnsLinkHint')}</p>
   <div className="sync-form-row"><label><span className="visually-hidden">{t('vnsSubject')}</span><input value={subject} onChange={e=>setSubject(e.target.value)} placeholder="https://vndb.org/v…" disabled={busy}/></label><button disabled={busy||!subject.trim()}>{t('vnsLink')}</button></div>
  </form>}
  {error&&<p role="alert" className="error-text">{error}</p>}
 </section>;
}
