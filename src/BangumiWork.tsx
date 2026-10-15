import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,type Work} from './api';
export function BangumiWork({work,onChanged,onHidden}:{work:Work;onChanged:()=>Promise<void>;onHidden:()=>void}){
 const {t}=useTranslation();const [subject,setSubject]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState('');
 async function run(path:string,body:object,hidden=false){setBusy(true);setError('');try{await api(path,body);await onChanged();if(hidden)onHidden();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 return <section className="section"><h2>{t('bgmTitle')}</h2>{work.resources===0&&<p className="pill">{t('availability_no_resources')}</p>}
  {work.bangumi_id?<><p><a href={`https://bgm.tv/subject/${work.bangumi_id}`} target="_blank" rel="noreferrer">Bangumi #{work.bangumi_id}</a> · {t(work.bangumi_hidden?'bgmKind_hidden':'bgmLinked')}</p><div className="inline"><button disabled={busy} onClick={()=>void run('/bangumi/link',{work_id:work.id,revision:work.revision,subject:''})}>{t('bgmUnlink')}</button>{work.resources===0&&<button disabled={busy} onClick={()=>void run('/bangumi/hide',{work_id:work.id,hidden:!work.bangumi_hidden},!work.bangumi_hidden)}>{t(work.bangumi_hidden?'bgmRestore':'bgmHide')}</button>}</div></>:<form onSubmit={e=>{e.preventDefault();void run('/bangumi/link',{work_id:work.id,revision:work.revision,subject});}}><p>{t('bgmLinkHint')}</p><label>{t('bgmSubject')}<input value={subject} onChange={e=>setSubject(e.target.value)} placeholder="https://bgm.tv/subject/…" disabled={busy}/></label><button disabled={busy||!subject.trim()}>{t('bgmLink')}</button></form>}
  {error&&<p role="alert" className="error-text">{error}</p>}
 </section>;
}
