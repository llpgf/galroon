import {TaskMessages} from './TaskMessages';
import {useEffect,useState} from 'react';
import {RefreshCw,Pause,Play,Square,Radio,ScanLine,BookOpen,Bell,Check,X} from 'lucide-react';
import {useTranslation} from 'react-i18next';
import {api,SessionChangedError,type Job} from './api';

type Run={id:string;state:string;processed:number;total:number;matched:number;review:number;skipped:number;current:string;message:string};
type Update={seq:number;job_id:string;kind:string;count:number;created:number;source:string;data:{matched?:number;review?:number}};
type Status={enabled:boolean;waiting:boolean;job:Run|null;watch:{total:number;watching:number;unavailable:number;reconciling:number;pending:number};updates:Update[]};
export function AutoMatchPanel({jobs,libraryId,workCount,canWrite,onCollection,onReview,onSources}:{jobs:Job[];libraryId:string;workCount:number;canWrite:boolean;onCollection:()=>void;onReview:()=>void;onSources:()=>void}){
 const {t}=useTranslation();const [status,setStatus]=useState<Status|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState(''),[open,setOpen]=useState(false),[expanded,setExpanded]=useState(false);
 const storageKey='galroon.updates.'+libraryId;const [seen,setSeen]=useState(()=>Number(localStorage.getItem(storageKey)||0));
 useEffect(()=>{let live=true,pending=false;const read=async()=>{if(pending)return;pending=true;try{const value=await api<Status>('/matching');if(live){setStatus(value);setError('');}}catch(e){if(live&&!(e instanceof SessionChangedError))setError((e as Error).message);}finally{pending=false;}};void read();const timer=setInterval(()=>void read(),1200);return()=>{live=false;clearInterval(timer);};},[]);
 const act=async(path:string)=>{setBusy(true);setError('');try{await api(path,{});setStatus(await api<Status>('/matching'));}catch(e){if(!(e instanceof SessionChangedError))setError((e as Error).message);}finally{setBusy(false);}};
 const job=status?.job,watch=status?.watch;const active=!!job&&['queued','running','pausing','cancelling'].includes(job.state);const percent=job&&job.total?Math.min(100,100*job.processed/job.total):0;
 const scan=jobs.find(j=>j.kind==='scan'&&['running','queued','pausing','cancelling'].includes(j.state));const updates=status?.updates||[];const unread=updates.filter(u=>u.seq>seen);const latest=unread[0];
 const acknowledge=()=>{const seq=updates[0]?.seq||0;setSeen(seq);localStorage.setItem(storageKey,String(seq));};
 const describe=(u:Update)=>u.kind==='resource.discovered'?t('intakeFound',{count:u.count,source:u.source}):t('intakeFinished',{matched:u.data.matched||0,review:u.data.review||0});
 const waiting=!!scan||status?.waiting;
 return <section className="auto-match" aria-label={t('intakeTitle')}>
  <div className="auto-match-heading"><div><h2>{t('sourceMatching')}</h2><small className="intake-summary">{t('intakeWatching',{count:watch?.watching||0,total:watch?.total||0})}</small></div><div className="inline"><button aria-expanded={expanded} onClick={()=>setExpanded(!expanded)}>{t(expanded?'hideProgress':'activityDetails')}</button><button aria-expanded={open} onClick={()=>{setOpen(!open);if(!open)acknowledge();}}><Bell size={15}/>{t('intakeUpdates')}{unread.length>0&&<span className="update-badge">{unread.length}</span>}</button>{canWrite&&<button disabled={busy||active} onClick={()=>void act('/matching/refresh')}><RefreshCw size={15}/>{t('refreshMatches')}</button>}</div></div>
  {expanded&&<ol className="intake-steps" aria-label={t('intakeStages')}>
   <li className={watch?.watching?'live':''}><Radio size={18}/><span><strong>{t('intakeWatch')}</strong><small>{t('intakeWatching',{count:watch?.watching||0,total:watch?.total||0})}</small></span></li>
   <li className={scan?'live':''}><ScanLine size={18}/><span><strong>{t('intakeScan')}</strong><small>{scan?t('intakeScanned',{count:scan.processed}):watch?.pending||watch?.reconciling?t('intakeScanWaiting'):t('intakeWatchingChanges')}</small></span></li>
   <li className={active?'live':''}><RefreshCw size={18} className={active?'spin':''}/><span><strong>{t('intakeMatch')}</strong><small>{active?t('matchChecked',{done:job!.processed,total:job!.total}):waiting?t('intakeWaitingScan'):job?t(job.state):t('intakeReady')}</small></span></li>
   <li><BookOpen size={18}/><span><strong>{t('intakeCollection')}</strong><small>{t('intakeWorks',{count:workCount})}</small></span></li>
  </ol>}
  {(scan||waiting)&&<p className="intake-summary" role="status">{scan?t('intakeScanned',{count:scan.processed}):t('intakeWaitingScan')}</p>}
  {watch&&watch.unavailable>0&&<p className="watch-attention">{t('intakeOffline',{count:watch.unavailable})} <button className="text-button" onClick={onSources}>{t('sources')}</button></p>}
  {latest&&!open&&<div className="intake-notice" role="status"><Bell size={16}/><span>{describe(latest)}</span><button onClick={latest.kind==='resource.discovered'&&canWrite?onReview:onCollection}>{t(latest.kind==='resource.discovered'&&canWrite?'intakeViewResources':'intakeViewCollection')}</button><button className="icon-button" aria-label={t('dismissUpdates')} onClick={acknowledge}><X size={15}/></button></div>}
  {open&&<div className="intake-updates"><div className="inline"><p>{t('intakeUpdatesHint')}</p>{unread.length>0&&<button onClick={acknowledge}>{t('markRead')}</button>}</div>{updates.length===0?<p>{t('intakeNoUpdates')}</p>:updates.map(u=><div className="intake-update" key={u.job_id+u.kind}>{u.kind==='resource.discovered'?<ScanLine size={16}/>:<Check size={16}/>}<div><p>{describe(u)}</p><time dateTime={new Date(u.created*1000).toISOString()}>{new Date(u.created*1000).toLocaleString()}</time></div><button onClick={u.kind==='resource.discovered'&&canWrite?onReview:onCollection}>{t(u.kind==='resource.discovered'&&canWrite?'intakeViewResources':'intakeViewCollection')}</button></div>)}</div>}
  {(active||job&&['paused','interrupted','failed','cancelled'].includes(job.state))&&<div className="intake-current">
   <div className="auto-match-counts" role="status"><span className={'pill '+job!.state}>{t(job!.state)}</span><span>{t('matchCount',{count:job!.matched})}</span><span>{t('matchReviewCount',{count:job!.review})}</span>{canWrite&&<div className="inline">{['paused','interrupted','failed','cancelled'].includes(job!.state)&&<button disabled={busy} onClick={()=>void act('/jobs/'+job!.id+'/resume')}><Play size={15}/>{t('resume')}</button>}{job!.state==='running'&&<button disabled={busy} onClick={()=>void act('/jobs/'+job!.id+'/pause')}><Pause size={15}/>{t('pause')}</button>}{['queued','running','paused'].includes(job!.state)&&<button disabled={busy} onClick={()=>void act('/jobs/'+job!.id+'/cancel')}><Square size={15}/>{t('stop')}</button>}</div>}</div>
   <div className="progress" role="progressbar" aria-label={t('autoMatchTitle')} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(percent)}><span style={{transform:'scaleX('+percent/100+')'}}/></div><TaskMessages activity={job!.current} message={job!.message} state={job!.state}/>
  </div>}
  {job?.state==='completed'&&job.review>0&&<p className="intake-review">{t('intakeReview',{count:job.review})} {canWrite&&<button className="text-button" onClick={onReview}>{t('intakeReviewAction')}</button>}</p>}
  {error&&<p role="alert" className="error-text">{error}</p>}
 </section>;
}
