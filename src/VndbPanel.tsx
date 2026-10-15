import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,readApi} from './api';
import {vndbValue,vndbChoice,defaultVndbAction,type VndbRow,type VndbState} from './vndb';

export function VndbRows({rows,choices,onChoice,onRestore,busy}:{rows:VndbRow[];choices:Record<string,string>;onChoice:(row:VndbRow,value:string)=>void;onRestore:(row:VndbRow)=>void;busy:boolean}){
 const {t}=useTranslation();
 return <div className="bgm-rows">{rows.map(row=><article className="bgm-row" key={row.subject_id+row.field}>
  <div><a href={`https://vndb.org/v${row.subject_id}`} target="_blank" rel="noreferrer">{row.title}</a><p>{t('vnsField_'+row.field)} · {t('vnsKind_'+row.kind)}</p><small>{t('vnsLocal')}: {vndbValue(row.field,row.local,t)} · VNDB: {vndbValue(row.field,row.remote,t)}{row.field==='entry'&&<> · {t('vnsField_rating')}: {vndbValue('rating',row.remote_rating,t)}</>}</small></div>
  {row.kind==='hidden'?<button disabled={busy} onClick={()=>onRestore(row)}>{t('vnsRestore')}</button>:row.kind==='same'?<span className="pill">{t('vnsKind_same')}</span>:<select disabled={busy} aria-label={t('vnsChooseFor',{title:row.title,field:t('vnsField_'+row.field)})} value={choices[row.fingerprint]??defaultVndbAction(row)} onChange={e=>onChoice(row,e.target.value)}>
   <option value="">{t('vnsLeave')}</option>
   {row.work_id?<><option value="local">{t(row.remote===null?'vnsCreateRemote':'vnsUseLocal')}</option>{row.remote!==null&&row.kind!=='unsupported'&&<option value="remote">{t('vnsUseRemote')}</option>}</>:<><option value="import">{t('vnsImportCard')}</option>{row.candidates.map(candidate=><option key={candidate.id} value={'match:'+candidate.id}>{t('vnsMatch',{title:candidate.title})}</option>)}<option value="skip">{t('vnsIgnore')}</option></>}
  </select>}
 </article>)}</div>;
}
export function VndbPanel({onChanged}:{onChanged:()=>Promise<void>}){
 const {t}=useTranslation();const [state,setState]=useState<VndbState|null>(null),[token,setToken]=useState(''),[pending,setBusy]=useState(false),[error,setError]=useState(''),[choices,setChoices]=useState<Record<string,string>>({}),[showAll,setShowAll]=useState(false),[limit,setLimit]=useState(50);
 useEffect(()=>{const controller=new AbortController();void readApi<VndbState>('/vndb_sync',controller.signal).then(setState).catch(e=>{if(!controller.signal.aborted)setError(String(e.message||e));});return()=>controller.abort();},[]);
 const busy=pending||!!state?.running;
 useEffect(()=>{if(!state?.running)return;const controller=new AbortController();let timer:ReturnType<typeof setTimeout>;
  async function poll(){try{const next=await readApi<VndbState>('/vndb_sync',controller.signal);if(controller.signal.aborted)return;setState(next);if(!next.running)await onChanged();else timer=setTimeout(()=>void poll(),1500);}catch(e){if(!controller.signal.aborted){setError(e instanceof Error?e.message:String(e));timer=setTimeout(()=>void poll(),3000);}}}
  timer=setTimeout(()=>void poll(),1000);return()=>{controller.abort();clearTimeout(timer);};
 },[state?.running,onChanged]);
 async function run(path:string,body:unknown={}){setBusy(true);setError('');try{const next=await api<VndbState>(path,body);setState(next);setChoices({});setLimit(50);if(path==='/vndb_sync/connect')setToken('');await onChanged();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 const rows=state?.rows||[],visible=rows.filter(row=>showAll||!['same','ignored','hidden','skipped'].includes(row.kind));
 const selected=rows.map(row=>({row,action:choices[row.fingerprint]??defaultVndbAction(row)})).filter(v=>v.action);
 return <section className="section bgm-panel vndb-panel"><h2>{t('vnsTitle')}</h2><p>{t('vnsHint')}</p>
  {state?.running&&<p role="status">{t('vnsWorking')}</p>}
  {error&&<p role="alert" className="error-text">{error}</p>}
  {!state&&!error&&<p role="status">{t('loading')}</p>}
  {state&&!state.account&&<form onSubmit={e=>{e.preventDefault();void run('/vndb_sync/connect',{token});}}><p><a href="https://vndb.org/u/tokens" target="_blank" rel="noreferrer">{t('vnsGetToken')}</a></p><label>{t('vnsToken')}<input type="password" autoComplete="off" value={token} onChange={e=>setToken(e.target.value)} disabled={busy}/></label><button className="primary" disabled={busy||!token.trim()}>{t('vnsConnect')}</button></form>}
  {state?.account&&<>
   <div className="inline"><strong>{state.account.user.username}</strong><span>@{state.account.user.username}</span><button disabled={busy} onClick={()=>void run('/vndb_sync/disconnect')}>{t('vnsDisconnect')}</button></div>
   <p>{t('vnsLastSync')}: {state.account.last_sync?new Date(state.account.last_sync*1000).toLocaleString():t('vnsNever')}</p>
   {state.account.error&&<p role="alert" className="error-text">{state.account.error}</p>}
   <div className="bgm-settings"><label>{t('vnsMode')}<select disabled={busy} value={state.account.mode} onChange={e=>void run('/vndb_sync/settings',{mode:e.target.value,automatic:state.account!.automatic})}><option value="full">{t('vnsFull')}</option><option value="existing">{t('vnsExisting')}</option></select></label>
   <label className="checkbox-label"><input type="checkbox" disabled={busy||!state.account.initialized} checked={state.account.automatic} onChange={e=>void run('/vndb_sync/settings',{mode:state.account!.mode,automatic:e.target.checked})}/>{t('vnsAutomatic')}</label></div>
   <p>{t('vnsAutomaticHint')}</p><p>{t('vnsPrivateHint')}</p>
   <div className="inline"><button disabled={busy} onClick={()=>void run('/vndb_sync/preview')}>{t(busy?'vnsWorking':'vnsPreview')}</button>{state.preview&&<button className="primary" disabled={busy} onClick={()=>void run('/vndb_sync/apply',{preview:state.preview,choices:selected.map(({row,action})=>vndbChoice(row,action))})}>{t('vnsApply',{count:selected.length})}</button>}</div>
   {state.results&&<p role="status">{t('vnsResults',{count:state.results.filter(r=>r.ok).length})}{state.results.filter(r=>!r.ok).map(r=><span key={r.subject_id+r.field} className="error-text"> {r.subject_id}: {r.error}</span>)}</p>}
   <label className="checkbox-label"><input type="checkbox" checked={showAll} onChange={e=>{setShowAll(e.target.checked);setLimit(50);}}/>{t('vnsShowAll')}</label>
   {state.preview&&<p>{t('vnsPreviewHint')}</p>}
   <VndbRows rows={visible.slice(0,limit)} choices={choices} busy={busy||!state.preview} onChoice={(row,value)=>setChoices(previous=>({...previous,[row.fingerprint]:value}))} onRestore={row=>void run('/vndb_sync/hide',{work_id:row.work_id,hidden:false})}/>
   {visible.length>limit&&<button onClick={()=>setLimit(limit+50)}>{t('vnsShowMore')}</button>}
   {state.preview&&!visible.length&&<p>{t('vnsNoChanges')}</p>}
  </>}
 </section>;
}
