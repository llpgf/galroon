import {useEffect,useState} from 'react';
import {Languages,RefreshCw,Trash2} from 'lucide-react';
import {useTranslation} from 'react-i18next';
import {api,deleteApi,readApi} from './api';
import {plainDescription} from './exploration';

export type TranslationRecord={
 target_kind:'work'|'character';target_id:string;target_lang:string;source_hash:string;
 translated_text:string;provider:string;model:string;updated:number;stale:boolean;
};
type TranslationResponse={configured:boolean;target_language:string;translation:TranslationRecord|null};

export function TranslatedDescription({kind,id,sourceText,spoilers,canWrite,collapsible=false,emptyFallback}:{kind:'work'|'character';id:string;sourceText:string;spoilers:boolean;canWrite:boolean;collapsible?:boolean;emptyFallback?:React.ReactNode}){
 const {t}=useTranslation();
 const [state,setState]=useState<TranslationResponse|null>(null);
 const [showOriginal,setShowOriginal]=useState(false);
 const [expanded,setExpanded]=useState(false);
 const [busy,setBusy]=useState(false);
 const [error,setError]=useState('');
 const raw=(sourceText||'').trim();

 useEffect(()=>{
  setState(null);setShowOriginal(false);setExpanded(false);setError('');
  if(!raw)return;
  const controller=new AbortController();
  void readApi<TranslationResponse>(`/translations/${kind}/${encodeURIComponent(id)}`,controller.signal)
   .then(res=>{if(!controller.signal.aborted)setState(res);})
   .catch(()=>{/* Keep original text if translation lookup fails. */});
  return()=>controller.abort();
 },[kind,id,raw]);

 const translate=async(force:boolean)=>{
  setBusy(true);setError('');
  try{
   const res=await api<TranslationResponse>(`/translations/${kind}/${encodeURIComponent(id)}`,{
    ...(kind==='character'?{source_text:raw}:{}),
    force,
   });
   setState(res);setShowOriginal(false);
  }catch(e){setError(e instanceof Error?e.message:String(e));}
  finally{setBusy(false);}
 };
 const remove=async()=>{
  setBusy(true);setError('');
  try{
   await deleteApi(`/translations/${kind}/${encodeURIComponent(id)}`);
   setState(prev=>prev?{...prev,translation:null}:null);setShowOriginal(false);
  }catch(e){setError(e instanceof Error?e.message:String(e));}
  finally{setBusy(false);}
 };

 const originalPlain=plainDescription(raw,spoilers);
 if(!originalPlain)return <>{emptyFallback||null}</>;
 const record=state?.translation||null;
 const showingTranslated=!!record&&!showOriginal;
 const displayText=showingTranslated?plainDescription(record.translated_text,spoilers):originalPlain;

 return <>
  <p className={'description'+(collapsible&&expanded?' expanded':'')}>{displayText}</p>
  {collapsible&&displayText.length>360&&<button className="text-button description-toggle" aria-expanded={expanded} onClick={()=>setExpanded(!expanded)}>{t(expanded?'showLess':'readMore')}</button>}
  <div className="inline translation-bar">
   {record&&<>
    <button className="text-button" onClick={()=>setShowOriginal(!showOriginal)}><Languages size={14}/>{t(showOriginal?'translationShowTranslated':'translationShowOriginal')}</button>
    <small className="muted">{record.provider} · {record.model}</small>
    {record.stale&&<span className="pill">{t('translationStale')}</span>}
   </>}
   {canWrite&&state?.configured&&<>
    <button className="text-button" disabled={busy} onClick={()=>void translate(!!record)}>
     {busy?<RefreshCw size={14} className="spin"/>:<Languages size={14}/>}
     {t(busy?'translating':record?'retranslate':'translateDescription')}
    </button>
    {record&&<button className="text-button" disabled={busy} aria-label={t('translationDelete')} title={t('translationDelete')} onClick={()=>void remove()}><Trash2 size={13}/></button>}
   </>}
  </div>
  {error&&<p className="error-text" role="alert">{error}</p>}
 </>;
}
