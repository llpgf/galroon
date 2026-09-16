import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ShieldCheck} from 'lucide-react';
import {api,bytes,type Plan} from './api';
export function PlanReview({plan,canWrite,onChanged}:{plan:Plan;canWrite:boolean;onChanged:(plan:Plan)=>void}){
 const {t}=useTranslation();const [busy,setBusy]=useState(false),[error,setError]=useState('');
 const act=async(action:'approve'|'execute'|'undo'|'open',id=plan.id)=>{setBusy(true);setError('');try{
  let target=id;if(action!=='open'){const result=await api<{id:string}>(`/plans/${id}/${action}`,action==='approve'?{digest:plan.digest}:{});target=result.id;}
  onChanged(await api<Plan>(`/plans/${target}`));
 }catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}};
 return <>
  <div className="eyebrow">{t('filePlan')}</div><h2>{t(plan.kind==='undo_organize'?'undoPlanTitle':'planTitle')}</h2><p>{t(plan.kind==='undo_organize'?'undoPlanHint':'planBody')}</p>
  {plan.kind==='organize'&&<p>{t('managedRootContract')}</p>}
  <div className="plan-context"><span className={`pill ${plan.state}`}>{t(plan.state)}</span><span>{t('planKind_'+plan.kind)}</span>{plan.created&&<time dateTime={new Date(plan.created*1000).toISOString()}>{new Date(plan.created*1000).toLocaleString()}</time>}</div>
  {plan.undo_of&&<button disabled={busy} onClick={()=>void act('open',plan.undo_of!)}>{t('openOriginalPlan')}</button>}
  {plan.reversed_by&&<p>{t('moveUndone')} <button disabled={busy} onClick={()=>void act('open',plan.reversed_by!)}>{t('openUndoPlan')}</button></p>}
  {plan.items.map(item=><div className="plan-item" key={item.id}><span>{t('from')}</span><p>{item.source}</p><span>{t('to')}</span><p>{item.target}</p><small>{bytes(item.size)}</small>{plan.operations?.find(o=>o.id===item.id)?.error&&<p className="error-text">{plan.operations.find(o=>o.id===item.id)?.error}</p>}</div>)}
  {canWrite&&plan.kind==='organize'&&plan.state==='completed'&&!plan.reversed_by&&<section className="undo-plan"><h3>{t('undoMove')}</h3><p>{t(plan.has_origin?'undoMoveHint':'legacyUndoUnavailable')}</p>{plan.has_origin&&<button disabled={busy} onClick={()=>void act('undo')}>{t('previewUndo')}</button>}</section>}
  {error&&<p className="error-text" role="alert">{error}</p>}
  <footer>{canWrite&&plan.state==='ready'&&<button className="primary" disabled={busy} onClick={()=>void act('approve')}><ShieldCheck size={16}/>{t('approve')}</button>}{canWrite&&['approved','partial'].includes(plan.state)&&<button className="primary" disabled={busy} onClick={()=>void act('execute')}>{t('execute')}</button>}</footer>
 </>;
}
