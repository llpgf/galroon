import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,ApiRequestError,SessionChangedError,type MatchSuggestion} from './api';
import {loadVerifiedSuggestions,confirmVerifiedSuggestions,type SuggestionBatchResult} from './suggestionRequests';

export function useSuggestionActions(canWrite:boolean,generation:number,onChanged:()=>Promise<void>){
 const {t}=useTranslation(),[verified,setVerified]=useState<MatchSuggestion[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState(''),[loadError,setLoadError]=useState(''),[results,setResults]=useState<SuggestionBatchResult[]>([]),[attempt,setAttempt]=useState(0),sending=useRef(false);
 useEffect(()=>{const controller=new AbortController();setVerified([]);setLoadError('');if(canWrite)void loadVerifiedSuggestions(controller.signal).then(items=>{if(!controller.signal.aborted)setVerified(items);}).catch(e=>{if(!controller.signal.aborted&&!(e instanceof SessionChangedError))setLoadError(e instanceof Error?e.message:String(e));});return()=>controller.abort();},[canWrite,generation,attempt]);
 const run=async(fn:()=>Promise<void>)=>{if(!canWrite||sending.current)return;sending.current=true;setBusy(true);setError('');setResults([]);try{await fn();}catch(e){if(!(e instanceof SessionChangedError))setError(e instanceof ApiRequestError&&e.status===409&&e.code==='suggestion_changed'?t('suggestionConflict'):e instanceof Error?e.message:String(e));}finally{try{await onChanged();}catch(e){if(!(e instanceof SessionChangedError))setLoadError(e instanceof Error?e.message:String(e));}setAttempt(n=>n+1);sending.current=false;setBusy(false);}};
 const confirm=(s:MatchSuggestion)=>run(()=>api(`/suggestions/${encodeURIComponent(s.id)}/confirm`,{revision:s.resource_revision}).then(()=>{}));
 const dismiss=(s:MatchSuggestion)=>run(()=>api(`/suggestions/${encodeURIComponent(s.id)}/dismiss`,{}).then(()=>{}));
 const batch=()=>run(()=>confirmVerifiedSuggestions(verified,items=>setResults(old=>[...old,...items])));
 return {verified,busy,error,loadError,results,confirm,dismiss,batch,reload:()=>setAttempt(n=>n+1)};
}
type Actions=ReturnType<typeof useSuggestionActions>;
export function SuggestionCard({suggestion:s,canWrite,actions}:{suggestion:MatchSuggestion;canWrite:boolean;actions:Actions}){
 const {t}=useTranslation();
 return <section className="match-suggestion" aria-label={t('suggestionBadge')}>
  <span className={'pill suggestion-badge '+(s.verified?'verified':'unverified')}>{t('suggestionBadge')} · {t(s.verified?'suggestionVerified':'suggestionUnverified')}</span>
  <ul className="suggestion-works">{s.works.map(w=><li key={w.vndb_id}>{w.title} <span className="muted">({w.vndb_id})</span></li>)}</ul>
  <details><summary>{t('suggestionRationale')}</summary><p className="suggestion-rationale">{s.rationale}</p></details>
  {canWrite&&<div className="inline"><button className="primary" disabled={actions.busy} onClick={()=>void actions.confirm(s)}>{t('suggestionConfirm')}</button><button disabled={actions.busy} onClick={()=>void actions.dismiss(s)}>{t('suggestionDismiss')}</button></div>}
 </section>;
}
export function SuggestionActions({actions,canWrite}:{actions:Actions;canWrite:boolean}){
 const {t}=useTranslation();
 return <div className="suggestion-actions">
  {canWrite&&actions.verified.length>0&&<div className="inline"><button disabled={actions.busy} onClick={()=>void actions.batch()}>{t('suggestionConfirmVerified')}</button><span className="muted">{t('suggestionVerifiedCount',{count:actions.verified.length})}</span></div>}
  {actions.error&&<p role="alert" className="error-text">{actions.error}</p>}
  {actions.loadError&&<div role="alert"><p>{actions.loadError}</p><button disabled={actions.busy} onClick={actions.reload}>{t('retry')}</button></div>}
  {actions.results.length>0&&<section className="suggestion-results" role="status" aria-label={t('suggestionResults')}><h3>{t('suggestionResults')}</h3><ul>{actions.results.map(({suggestion:s,result})=><li key={result.id}>{s.works.map(w=>`${w.title} (${w.vndb_id})`).join(' · ')} — {t('state' in result?'suggestionConfirmed':'suggestionFailed','error' in result?{error:result.error}:{})}</li>)}</ul></section>}
 </div>;
}
