import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,readApi} from './api';
import {bangumiStatus,bangumiChoice,defaultBangumiAction,type BangumiRow,type BangumiState} from './bangumi';

export function BangumiRows({rows,choices,onChoice,onRestore,busy}:{rows:BangumiRow[];choices:Record<string,string>;onChoice:(row:BangumiRow,value:string)=>void;onRestore:(row:BangumiRow)=>void;busy:boolean}){
 const {t}=useTranslation();
 return <div className="bgm-rows">{rows.map(row=><article className="bgm-row" key={row.subject_id}>
  <div><a href={`https://bgm.tv/subject/${row.subject_id}`} target="_blank" rel="noreferrer">{row.title}</a><p>{t('bgmKind_'+row.kind+(row.kind==='match'&&row.suggested?'_suggested':''))}</p><small>{t('bgmLocal')}: {t(bangumiStatus(row.local))} · Bangumi: {t(bangumiStatus(row.remote))}</small></div>
  {row.kind==='hidden'?<button disabled={busy} onClick={()=>onRestore(row)}>{t('bgmRestore')}</button>:row.kind==='same'?<span className="pill">{t('bgmKind_same')}</span>:<select disabled={busy} aria-label={t('bgmChooseFor',{title:row.title})} value={choices[row.fingerprint]??defaultBangumiAction(row)} onChange={e=>onChoice(row,e.target.value)}>
   <option value="">{t(row.kind==='cleared'?'bgmIgnoreCleared':'bgmLeave')}</option>
   {row.work_id?<>{row.local!==0&&<option value="local">{t(row.remote===null?'bgmCreateRemote':'bgmUseLocal')}</option>}{row.remote!==null&&<option value="remote">{t(row.kind==='cleared'?'bgmUseRemoteCleared':'bgmUseRemote')}</option>}</>:<><option value="import">{t('bgmImportCard')}</option>{row.candidates.map(candidate=><option key={candidate.id} value={'match:'+candidate.id}>{t('bgmMatch',{title:candidate.title})}</option>)}<option value="skip">{t('bgmIgnore')}</option></>}
  </select>}
 </article>)}</div>;
}
export function BangumiPanel({onChanged}:{onChanged:()=>Promise<void>}){
 const {t}=useTranslation();const [state,setState]=useState<BangumiState|null>(null),[token,setToken]=useState(''),[busy,setBusy]=useState(false),[error,setError]=useState(''),[choices,setChoices]=useState<Record<string,string>>({}),[showAll,setShowAll]=useState(false),[limit,setLimit]=useState(50);
 useEffect(()=>{const controller=new AbortController();void readApi<BangumiState>('/bangumi',controller.signal).then(setState).catch(e=>{if(!controller.signal.aborted)setError(String(e.message||e));});return()=>controller.abort();},[]);
 async function run(path:string,body:unknown={}){setBusy(true);setError('');try{const next=await api<BangumiState>(path,body);setState(next);setChoices({});setLimit(50);if(path==='/bangumi/connect')setToken('');await onChanged();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 const rows=state?.rows||[],visible=rows.filter(row=>showAll||!['same','ignored','hidden','skipped'].includes(row.kind));
 const selected=rows.map(row=>({row,action:choices[row.fingerprint]??defaultBangumiAction(row)})).filter(v=>v.action);
 return <section className="section bgm-panel"><h2>{t('bgmTitle')}</h2><p>{t('bgmHint')}</p>
  {error&&<p role="alert" className="error-text">{error}</p>}
  {!state&&!error&&<p role="status">{t('loading')}</p>}
  {state&&!state.account&&<form onSubmit={e=>{e.preventDefault();void run('/bangumi/connect',{token});}}><p><a href="https://next.bgm.tv/demo/access-token" target="_blank" rel="noreferrer">{t('bgmGetToken')}</a></p><label>{t('bgmToken')}<input type="password" autoComplete="off" value={token} onChange={e=>setToken(e.target.value)} disabled={busy}/></label><button className="primary" disabled={busy||!token.trim()}>{t('bgmConnect')}</button></form>}
  {state?.account&&<>
   <div className="inline"><strong>{state.account.user.nickname||state.account.user.username}</strong><span>@{state.account.user.username}</span><button disabled={busy} onClick={()=>void run('/bangumi/disconnect')}>{t('bgmDisconnect')}</button></div>
   <p>{t('bgmLastSync')}: {state.account.last_sync?new Date(state.account.last_sync*1000).toLocaleString():t('bgmNever')}</p>
   {state.account.error&&<p role="alert" className="error-text">{state.account.error}</p>}
   <div className="bgm-settings"><label>{t('bgmMode')}<select disabled={busy} value={state.account.mode} onChange={e=>void run('/bangumi/settings',{mode:e.target.value,automatic:state.account!.automatic})}><option value="full">{t('bgmFull')}</option><option value="existing">{t('bgmExisting')}</option></select></label>
   <label className="checkbox-label"><input type="checkbox" disabled={busy||!state.account.initialized} checked={state.account.automatic} onChange={e=>void run('/bangumi/settings',{mode:state.account!.mode,automatic:e.target.checked})}/>{t('bgmAutomatic')}</label></div>
   <p>{t('bgmAutomaticHint')}</p><p>{t('bgmPrivateHint')}</p>
   <div className="inline"><button disabled={busy} onClick={()=>void run('/bangumi/preview')}>{t(busy?'bgmWorking':'bgmPreview')}</button>{state.preview&&<button className="primary" disabled={busy} onClick={()=>void run('/bangumi/apply',{preview:state.preview,choices:selected.map(({row,action})=>bangumiChoice(row,action))})}>{t('bgmApply',{count:selected.length})}</button>}</div>
   {state.results&&<p role="status">{t('bgmResults',{count:state.results.filter(r=>r.ok).length})}{state.results.filter(r=>!r.ok).map(r=><span key={r.subject_id} className="error-text"> {r.subject_id}: {r.error}</span>)}</p>}
   <label className="checkbox-label"><input type="checkbox" checked={showAll} onChange={e=>{setShowAll(e.target.checked);setLimit(50);}}/>{t('bgmShowAll')}</label>
   {state.preview&&<p>{t('bgmPreviewHint')}</p>}
   <BangumiRows rows={visible.slice(0,limit)} choices={choices} busy={busy||!state.preview} onChoice={(row,value)=>setChoices(previous=>({...previous,[row.fingerprint]:value}))} onRestore={row=>void run('/bangumi/hide',{work_id:row.work_id,hidden:false})}/>
   {visible.length>limit&&<button onClick={()=>setLimit(limit+50)}>{t('bgmShowMore')}</button>}
   {state.preview&&!visible.length&&<p>{t('bgmNoChanges')}</p>}
  </>}
 </section>;
}
