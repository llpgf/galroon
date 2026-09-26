import {TaskMessages} from './TaskMessages';
import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api,bytes,type Job,type Plan} from './api';
type Entry={seq:number;job_id:string;code:string;created:number;details:Record<string,string|number|boolean|null>};
type Page={items:Entry[];next:number|null};
type Value=Entry['details'][string];
const ID_FIELDS=new Set(['resource_id','work_id','plan_id','operation_id','issue_id','undo_of']);
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i;
/** Long identifiers stay available (hover or copy) without dominating the entry. */
function ShortId({value}:{value:string}){return <code className="timeline-id" title={value}>{UUID.test(value)?value.slice(0,8)+'…':value}</code>;}
export function Timeline({onPlan}:{onPlan?:(plan:Plan)=>void}){const {t,i18n}=useTranslation(),generation=useRef(0);const [items,setItems]=useState<Entry[]>([]),[next,setNext]=useState<number|null>(null),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const [task,setTask]=useState<Job|null>(null),[selectedTask,setSelectedTask]=useState<number|null>(null);
 const detailGeneration=useRef(0);
 async function openPlan(id:string){const gen=++detailGeneration.current;setBusy(true);setError('');try{const plan=await api<Plan>(`/plans/${encodeURIComponent(id)}`);if(gen===detailGeneration.current)onPlan?.(plan);}catch(e){if(gen===detailGeneration.current)setError(e instanceof Error?e.message:String(e));}finally{if(gen===detailGeneration.current)setBusy(false);}}
 async function openTask(id:string,seq:number){const gen=++detailGeneration.current;setBusy(true);setError('');setSelectedTask(seq);setTask(null);try{const value=await api<Job>(`/jobs/${encodeURIComponent(id)}`);if(gen===detailGeneration.current)setTask(value);}catch(e){if(gen===detailGeneration.current)setError(e instanceof Error?e.message:String(e));}finally{if(gen===detailGeneration.current)setBusy(false);}}
 async function load(cursor:number|null){const gen=generation.current;setBusy(true);setError('');try{const p=await api<Page>(`/timeline${cursor===null?'':`?before=${cursor}`}`);if(gen!==generation.current)return;setItems(old=>cursor===null?p.items:[...old,...p.items]);setNext(p.next);}catch(e){if(gen===generation.current)setError(e instanceof Error?e.message:String(e));}finally{if(gen===generation.current)setBusy(false);}}
 function show(code:string,key:string,value:Value){
  if(value===null)return '—';
  if(typeof value==='boolean')return t(value?'timelineYes':'timelineNo');
  const text=String(value);
  if(key==='bytes'&&typeof value==='number')return bytes(value);
  if(key==='kind'&&code.startsWith('file.'))return t('planKind_'+text,{defaultValue:text});
  // States and job-control transitions reuse the task and matching labels; work references in manual matches stay identifiers.
  if(key==='state'||key==='action'||(['from','to'].includes(key)&&(code.startsWith('file.')||code==='job.control')))return code.startsWith('match.')?t('matchState_'+text,{defaultValue:t(text,{defaultValue:text})}):t(text,{defaultValue:text});
  if(ID_FIELDS.has(key)||['from','to'].includes(key))return <ShortId value={text}/>;
  return typeof value==='number'?value.toLocaleString(i18n.language):text;
 }
 useEffect(()=>{generation.current++;void load(null);return()=>{generation.current++;detailGeneration.current++;};},[]);
 return <section className="section"><h2>{t('timeline')}</h2><p>{t('timelineHint')}</p><button disabled={busy} onClick={()=>void load(null)}>{t('refreshView')}</button>{items.map(item=><article className="plan-item" key={item.seq}><strong>{t('timeline_'+item.code.replaceAll('.','_'))}</strong> · <time>{new Date(item.created*1000).toLocaleString(i18n.language)}</time><dl className="timeline-details">{item.job_id&&<div><dt>{t('timelineJob')}</dt><dd><ShortId value={item.job_id}/></dd></div>}{Object.entries(item.details).map(([key,value])=><div key={key}><dt>{t('timelineField_'+key,{defaultValue:key})}</dt><dd>{show(item.code,key,value)}</dd></div>)}</dl><div className="inline">{onPlan&&typeof item.details.plan_id==='string'&&<button disabled={busy} onClick={()=>void openPlan(String(item.details.plan_id))}>{t('timelineOpenPlan')}</button>}{item.job_id&&<button disabled={busy} onClick={()=>void openTask(item.job_id,item.seq)}>{t('timelineOpenTask')}</button>}</div>{selectedTask===item.seq&&item.job_id&&task&&<section className="audit-task-detail" aria-label={t('timelineCurrentTask')}><h3>{t('timelineCurrentTask')}</h3><p>{t('timelineTaskSnapshotHint')}</p><p className="path">{task.id}</p><span className={'pill '+task.state}>{t(task.state)}</span><p>{t('processed')}: {task.processed.toLocaleString()} · {bytes(task.bytes)} · {t('errors')}: {task.errors}</p><TaskMessages activity={task.current_path} message={task.message} state={task.state}/><button disabled={busy} onClick={()=>void openTask(task.id,item.seq)}>{t('refreshView')}</button><button onClick={()=>{detailGeneration.current++;setSelectedTask(null);setTask(null);setBusy(false);}}>{t('close')}</button></section>}</article>)}{busy&&<p role="status">{t('loading')}</p>}{error&&<p role="alert">{error}</p>}{!busy&&!error&&items.length===0&&<p>{t('timelineEmpty')}</p>}{next!==null&&<button disabled={busy} onClick={()=>void load(next)}>{t('historyOlder')}</button>}</section>;
}
