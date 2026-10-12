import {SuggestionCard,SuggestionActions,useSuggestionActions} from './Suggestions';
import {useContext} from 'react';
import {CatalogGenerationContext} from './useOwnedWorks';
import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {CircleCheck,RefreshCw} from 'lucide-react';
import {api} from './api';
import {coreText} from './coreText';
import {IssueScan} from './IssueScan';
import {IssueTask} from './IssueTask';
import {useIssuePages,type Issue} from './useIssuePages';
export function IssuesPanel({onReview,onSource,canWrite,onChanged}:{canWrite:boolean;onChanged:()=>Promise<void>;onSource:()=>void;onReview:(resource:{id:string;title:string;path:string})=>void}){
 const catalogGeneration=useContext(CatalogGenerationContext);
 const {t}=useTranslation();const [filter,setFilter]=useState('open'),[changing,setChanging]=useState(false),[error,setError]=useState(''),[queued,setQueued]=useState(false);const [taskIssue,setTaskIssue]=useState<number|null>(null);const [scanIssue,setScanIssue]=useState<Issue|null>(null),[scanQueued,setScanQueued]=useState(false);
 const page=useIssuePages(filter,catalogGeneration),{items,next}=page,busy=changing||page.busy;
 useEffect(()=>{setError('');window.scrollTo(0,0);},[filter]);
 const suggestions=useSuggestionActions(canWrite,catalogGeneration,async()=>{await page.reload();await onChanged();});
 async function change(issue:Issue,state:string){setChanging(true);setError('');try{await api(`/issues/${issue.id}`,{revision:issue.revision,state});await page.reload();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setChanging(false);}}
 async function retry(issue:Issue){setChanging(true);setError('');setQueued(false);try{await api('/issues/'+issue.id+'/retry',{revision:issue.revision});await page.reload();setQueued(true);}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setChanging(false);}}
 return <section className="issues-panel"><header className="panel-heading"><h2 title={t('issueScope')}>{t('issueCenter')}</h2><div className="tabs">{['open','deferred','resolved'].map(state=><button key={state} disabled={busy} aria-pressed={filter===state} onClick={()=>setFilter(state)}>{t('issue_'+state)}</button>)}</div><button className="icon-button" disabled={busy} aria-label={t('refreshView')} title={t('refreshView')} onClick={()=>void page.reload()}><RefreshCw size={16}/></button></header>
 <SuggestionActions actions={suggestions} canWrite={canWrite}/>
 {items.map(issue=><article className="plan-item issue" key={issue.id}><div className="issue-head"><h3>{issue.title||issue.resource_id}</h3><span className="pill partial">{t('issueKind_'+issue.kind)}</span></div><p>{coreText(t,issue.reason)}</p><p className="path">{issue.source&&<>{issue.source} · </>}{issue.path}</p>{issue.suggestion?.state==='open'&&<SuggestionCard suggestion={issue.suggestion} canWrite={canWrite} actions={suggestions}/>}<div className="inline"><button onClick={()=>issue.category==='acquire'?setTaskIssue(issue.id):(issue.category==='availability'||issue.category==='source')?onSource():onReview({id:issue.resource_id||'',title:issue.title||'',path:issue.path||''})}>{t(issue.category==='acquire'?'issueTask':(issue.category==='availability'||issue.category==='source')?'issueViewSource':'matchingHistory')}</button>{filter!=='resolved'&&(issue.category==='availability'||issue.category==='source')&&<button disabled={busy||issue.retry_pending} onClick={()=>{setScanQueued(false);setScanIssue(issue);}}>{t('issueScan')}</button>}{filter!=='resolved'&&(issue.category!=='availability'&&issue.category!=='source'&&issue.category!=='acquire')&&<button disabled={busy||issue.retry_pending} onClick={()=>void retry(issue)}>{t('issueRetry')}</button>}{filter!=='resolved'&&<button disabled={busy} onClick={()=>void change(issue,filter==='deferred'?'open':'deferred')}>{t(filter==='deferred'?'issueReopen':'issueDefer')}</button>}</div></article>)}
 {taskIssue!==null&&<IssueTask id={taskIssue} onClose={()=>{setTaskIssue(null);void page.reload();}}/>}{scanIssue&&<IssueScan issue={scanIssue} onClose={()=>setScanIssue(null)} onQueued={()=>{setScanIssue(null);setScanQueued(true);void page.reload();}}/>}{scanQueued&&<p role="status">{t('issueScanQueued')}</p>}{queued&&<p role="status">{t('issueRetryQueued')}</p>}{(error||page.error)&&<p role="alert">{error||page.error}</p>}{busy&&items.length===0&&<p role="status">{t('loading')}</p>}{!error&&!page.error&&!busy&&items.length===0&&<p className="issues-empty"><CircleCheck size={16}/>{t(filter==='open'?'issueEmptyOpen':'issueEmpty')}</p>}{next!==null&&<button disabled={busy} onClick={()=>void page.more()}>{t('historyOlder')}</button>}</section>;
}




