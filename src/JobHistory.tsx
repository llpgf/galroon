import {type ReactNode,useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {readApi,type Job} from './api';

export type JobPage={items:Job[];next:string|null};
export function JobHistory({latest,render}:{latest:JobPage;render:(job:Job,changed:()=>Promise<void>)=>ReactNode}){
 const {t}=useTranslation();
 const [page,setPage]=useState<JobPage|null>(null),[cursor,setCursor]=useState<string|null>(null),[previous,setPrevious]=useState<(string|null)[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const request=useRef<AbortController|null>(null),heading=useRef<HTMLHeadingElement>(null),mounted=useRef(true);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;request.current?.abort();};},[]);
 async function load(next:string|null,history:(string|null)[],focus=true){
  request.current?.abort();const controller=new AbortController();request.current=controller;
  setBusy(true);setError('');
  try{
   const result=next===null?null:await readApi<JobPage>(`/jobs?paged=true&before=${encodeURIComponent(next)}`,controller.signal);
   if(controller.signal.aborted)return;
   setPage(result);setCursor(next);setPrevious(history);
   if(focus)requestAnimationFrame(()=>heading.current?.focus());
  }catch(e){if(!controller.signal.aborted)setError(e instanceof Error?e.message:String(e));}
  finally{if(!controller.signal.aborted)setBusy(false);}
 }
 const shown=page||latest;
 const observedRequest=request.current;
 const changed=async()=>{if(mounted.current&&request.current===observedRequest&&cursor!==null)await load(cursor,previous,false);};
 const controls=<><button disabled={busy||previous.length===0} onClick={()=>void load(previous.at(-1)??null,previous.slice(0,-1))}>{t('tasksNewer')}</button><button disabled={busy||shown.next===null} onClick={()=>void load(shown.next,[...previous,cursor])}>{t('historyOlder')}</button></>;
 return <section aria-busy={busy}>
  <h2 ref={heading} tabIndex={-1} className="task-history-title">{t(cursor===null?'tasksLatest':'tasksOlder')}</h2>
  <p className="muted">{t(cursor===null?'tasksLiveHint':'tasksHistoryHint')}</p>
  <nav className="inline" aria-label={t('taskPages')}>{controls}</nav>
  {cursor!==null&&<div className="inline"><button disabled={busy} onClick={()=>void load(null,[])}>{t('tasksLatest')}</button><button disabled={busy} onClick={()=>void load(cursor,previous,false)}>{t('refreshView')}</button></div>}
  {error&&<p role="alert">{error}</p>}
  {shown.items.length===0&&<p>{t('noTasks')}</p>}
  {shown.items.map(job=>render(job,changed))}
  <nav className="inline" aria-label={t('taskPages')}>{controls}</nav>
 </section>;
}
