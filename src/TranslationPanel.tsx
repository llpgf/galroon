import {useEffect,useState} from 'react';
import {Languages,RefreshCw,Play,Square,CheckCircle2} from 'lucide-react';
import {useTranslation} from 'react-i18next';
import {api} from './api';

export type LlmProvider='openai'|'gemini'|'claude';
export type TranslationSettings={
 provider:LlmProvider;
 base_url:string;
 default_base_url:string;
 model:string;
 default_model:string;
 target_language:string;
 system_prompt:string;
 keep_original_names:boolean;
 has_api_key:boolean;
 configured:boolean;
 untranslated_works:number;
 total_works:number;
 untranslated_characters:number;
 total_characters:number;
};
export type BatchScope='works'|'characters'|'all';
export type BatchStatus={
 running:boolean;scope?:string;total:number;translated:number;failed:number;skipped:number;
 current_title:string|null;last_error:string|null;started_at:number|null;finished_at:number|null;
};
export const PROVIDER_DEFAULTS:Record<LlmProvider,{baseUrl:string;model:string}>={
 openai:{baseUrl:'https://api.openai.com/v1/',model:'gpt-4o-mini'},
 gemini:{baseUrl:'https://generativelanguage.googleapis.com/v1beta/',model:'gemini-2.5-flash'},
 claude:{baseUrl:'https://api.anthropic.com/v1/',model:'claude-haiku-4-5'},
};

export function TranslationPanel({onChanged}:{onChanged?:()=>Promise<void>}){
 const {t}=useTranslation();
 const [settings,setSettings]=useState<TranslationSettings|null>(null);
 const [provider,setProvider]=useState<LlmProvider>('openai');
 const [baseUrl,setBaseUrl]=useState('');
 const [model,setModel]=useState('');
 const [apiKey,setApiKey]=useState('');
 const [targetLang,setTargetLang]=useState('zh-TW');
 const [systemPrompt,setSystemPrompt]=useState('');
 const [keepOriginalNames,setKeepOriginalNames]=useState(true);
 const [forceRetranslate,setForceRetranslate]=useState(false);
 const [batch,setBatch]=useState<BatchStatus|null>(null);
 const [busy,setBusy]=useState(false);
 const [testing,setTesting]=useState(false);
 const [testResult,setTestResult]=useState<{model:string;latency_ms:number;sample:string}|null>(null);
 const [message,setMessage]=useState('');
 const [error,setError]=useState('');

 const applyLoaded=(s:TranslationSettings)=>{
  setSettings(s);setProvider(s.provider);setBaseUrl(s.base_url);setModel(s.model);
  setTargetLang(s.target_language||'zh-TW');setSystemPrompt(s.system_prompt||'');
  setKeepOriginalNames(s.keep_original_names??true);
 };
 const load=async()=>{
  const [s,b]=await Promise.all([api<TranslationSettings>('/translation/settings'),api<BatchStatus>('/translations/batch')]);
  applyLoaded(s);setBatch(b);
 };
 useEffect(()=>{void load().catch(e=>setError(e instanceof Error?e.message:String(e)));},[]);
 useEffect(()=>{
  if(!batch?.running)return;
  const timer=setInterval(()=>{
   void api<BatchStatus>('/translations/batch').then(next=>{
    setBatch(next);
    if(!next.running){void load();void onChanged?.();}
   }).catch(()=>{});
  },1500);
  return()=>clearInterval(timer);
 },[batch?.running]);

 const act=async(fn:()=>Promise<void>)=>{
  setBusy(true);setError('');setMessage('');
  try{await fn();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}
 };
 const save=()=>act(async()=>{
  const saved=await api<TranslationSettings>('/translation/settings',{
   provider,base_url:baseUrl,model:model.trim()||PROVIDER_DEFAULTS[provider].model,
   target_language:targetLang,system_prompt:systemPrompt,keep_original_names:keepOriginalNames,
   ...(apiKey.trim()?{api_key:apiKey.trim()}:{}),
  });
  applyLoaded(saved);setApiKey('');setMessage(t('translationSaved'));
 });
 const clearKey=()=>act(async()=>{
  const saved=await api<TranslationSettings>('/translation/settings',{clear_api_key:true});
  applyLoaded(saved);setApiKey('');setMessage(t('translationKeyCleared'));
 });
 const test=async()=>{
  setTesting(true);setError('');setMessage('');setTestResult(null);
  try{
   const res=await api<{ok:boolean;model:string;latency_ms:number;sample:string}>('/translation/test',{
    provider,base_url:baseUrl,model:model.trim()||PROVIDER_DEFAULTS[provider].model,
    target_language:targetLang,keep_original_names:keepOriginalNames,...(apiKey.trim()?{api_key:apiKey.trim()}:{}),
   });
   setTestResult({model:res.model,latency_ms:res.latency_ms,sample:res.sample});
  }catch(e){setError(e instanceof Error?e.message:String(e));}
  finally{setTesting(false);}
 };
 const startBatch=(scope:BatchScope)=>act(async()=>{
  const next=await api<BatchStatus>('/translations/batch',{scope,force:forceRetranslate});
  setBatch(next);
  if(next.total===0)setMessage(t('translationBatchNone'));
 });
 const cancelBatch=()=>act(async()=>{
  const res=await api<{cancelled:boolean;status:BatchStatus}>('/translations/batch/cancel',{});
  setBatch(res.status);setMessage(t('translationBatchStopped'));
 });

 const defaults=PROVIDER_DEFAULTS[provider];
 return <section className="section translation-panel">
  <h2><Languages size={20}/> {t('translationTitle')}</h2>
  <p>{t('translationHint')}</p>
  {!settings?<p>{t('loading')}</p>:<>
   <div className="access-fields">
    <label>{t('translationProvider')}
     <select value={provider} disabled={busy} onChange={e=>{const p=e.target.value as LlmProvider;setProvider(p);if(!model||Object.values(PROVIDER_DEFAULTS).some(d=>d.model===model))setModel(PROVIDER_DEFAULTS[p].model);}}>
      <option value="openai">{t('translationProvider_openai')}</option>
      <option value="gemini">{t('translationProvider_gemini')}</option>
      <option value="claude">{t('translationProvider_claude')}</option>
     </select>
    </label>
    <label>{t('translationModel')}
     <input value={model} disabled={busy} placeholder={defaults.model} onChange={e=>setModel(e.target.value)}/>
    </label>
    <label>{t('translationBaseUrl')}
     <input value={baseUrl} disabled={busy} placeholder={defaults.baseUrl} onChange={e=>setBaseUrl(e.target.value)}/>
     <small className="muted">{t('translationBaseUrlHint')}</small>
    </label>
    <label>{t('translationApiKey')} {settings.has_api_key&&<span className="pill">{t('translationKeySaved')}</span>}
     <input type="password" autoComplete="off" value={apiKey} disabled={busy} placeholder={settings.has_api_key?t('translationKeyKeepPlaceholder'):t('translationKeyPlaceholder')} onChange={e=>setApiKey(e.target.value)}/>
    </label>
    <label>{t('translationTargetLang')}
     <select value={['zh-TW','zh-CN','en','ja'].includes(targetLang)?targetLang:'zh-TW'} disabled={busy} onChange={e=>setTargetLang(e.target.value)}>
      <option value="zh-TW">繁體中文 (zh-TW)</option>
      <option value="zh-CN">简体中文 (zh-CN)</option>
      <option value="en">English (en)</option>
      <option value="ja">日本語 (ja)</option>
     </select>
    </label>
    <label className="checkbox-label">
     <input type="checkbox" checked={keepOriginalNames} disabled={busy} onChange={e=>setKeepOriginalNames(e.target.checked)}/>
     <span><strong>{t('translationKeepOriginalNames')}</strong><small className="muted"> — {t('translationKeepOriginalNamesHint')}</small></span>
    </label>
    <details>
     <summary>{t('translationPromptTitle')}</summary>
     <label>{t('translationPromptHint')}
      <textarea rows={3} value={systemPrompt} disabled={busy} placeholder={t('translationPromptPlaceholder')} onChange={e=>setSystemPrompt(e.target.value)}/>
     </label>
    </details>
   </div>
   <div className="inline">
    <button className="primary" disabled={busy||testing} onClick={()=>void save()}>{t('save')}</button>
    <button disabled={busy||testing} onClick={()=>void test()}>{testing?<RefreshCw size={14} className="spin"/>:<CheckCircle2 size={14}/>}{t('translationTest')}</button>
    {settings.has_api_key&&<button disabled={busy||testing} onClick={()=>void clearKey()}>{t('translationClearKey')}</button>}
   </div>
   {testResult&&<div className="plan-item" role="status"><strong>{t('translationTestOk',{model:testResult.model,ms:testResult.latency_ms})}</strong><p>{testResult.sample}</p></div>}
   <div className="plan-item">
    <h3>{t('translationBatchTitle')}</h3>
    <p>{t('translationBatchSummary',{works:settings.untranslated_works,totalWorks:settings.total_works,characters:settings.untranslated_characters,totalCharacters:settings.total_characters})}</p>
    {batch?.running?<>
     <p role="status">{t('translationBatchProgress',{done:batch.translated+batch.failed,total:batch.total,failed:batch.failed})}{batch.current_title?` · ${batch.current_title}`:''}</p>
     <button disabled={busy} onClick={()=>void cancelBatch()}><Square size={14}/>{t('stop')}</button>
    </>:<>
     <label className="checkbox-label">
      <input type="checkbox" checked={forceRetranslate} disabled={busy} onChange={e=>setForceRetranslate(e.target.checked)}/>
      <span>{t('translationBatchForce')}</span>
     </label>
     <div className="inline">
      <button disabled={busy||!settings.configured||(!forceRetranslate&&settings.untranslated_works===0)} onClick={()=>void startBatch('works')}><Play size={14}/>{t('translationBatchStartWorks',{count:forceRetranslate?settings.total_works:settings.untranslated_works})}</button>
      <button disabled={busy||!settings.configured||(!forceRetranslate&&settings.untranslated_characters===0)} onClick={()=>void startBatch('characters')}><Play size={14}/>{t('translationBatchStartCharacters',{count:forceRetranslate?settings.total_characters:settings.untranslated_characters})}</button>
      <button disabled={busy||!settings.configured||(!forceRetranslate&&settings.untranslated_works+settings.untranslated_characters===0)} onClick={()=>void startBatch('all')}><Play size={14}/>{t('translationBatchStartAll')}</button>
     </div>
    </>}
    {batch&&!batch.running&&batch.finished_at&&batch.total>0&&<p className="muted">{t('translationBatchDone',{translated:batch.translated,failed:batch.failed})}{batch.last_error?` · ${batch.last_error}`:''}</p>}
   </div>
  </>}
  {message&&<p role="status">{message}</p>}
  {error&&<p className="error-text" role="alert">{error}</p>}
 </section>;
}
