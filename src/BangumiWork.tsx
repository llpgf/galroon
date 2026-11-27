import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ExternalLink} from 'lucide-react';
import {api,type Work} from './api';
export function BangumiWork({work,onChanged,onHidden}:{work:Work;onChanged:()=>Promise<void>;onHidden:()=>void}){
 const {t}=useTranslation();const [subject,setSubject]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState('');
 async function run(path:string,body:object,hidden=false){setBusy(true);setError('');try{await api(path,body);await onChanged();if(hidden)onHidden();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 return <section className="sync-card">
  <div className="sync-card-head"><h2>{t('bgmTitle')}</h2>{work.resources===0&&<span className="pill">{t('availability_no_resources')}</span>}</div>
  {work.bangumi_id?<div className="sync-card-body">
   <p><a href={`https://bgm.tv/subject/${work.bangumi_id}`} target="_blank" rel="noreferrer">Bangumi #{work.bangumi_id} <ExternalLink size={12}/></a> · {t(work.bangumi_hidden?'bgmKind_hidden':'bgmLinked')}</p>
   <div className="inline"><button disabled={busy} onClick={()=>void run('/bangumi/link',{work_id:work.id,revision:work.revision,subject:''})}>{t('bgmUnlink')}</button>{work.resources===0&&<button disabled={busy} onClick={()=>void run('/bangumi/hide',{work_id:work.id,hidden:!work.bangumi_hidden},!work.bangumi_hidden)}>{t(work.bangumi_hidden?'bgmRestore':'bgmHide')}</button>}</div>
  </div>:<form className="sync-form" onSubmit={e=>{e.preventDefault();void run('/bangumi/link',{work_id:work.id,revision:work.revision,subject});}}>
   <p className="muted">{t('bgmLinkHint')}</p>
   <div className="sync-form-row"><label><span className="visually-hidden">{t('bgmSubject')}</span><input value={subject} onChange={e=>setSubject(e.target.value)} placeholder="https://bgm.tv/subject/…" disabled={busy}/></label><button disabled={busy||!subject.trim()}>{t('bgmLink')}</button></div>
  </form>}
  {error&&<p role="alert" className="error-text">{error}</p>}
 </section>;
}
