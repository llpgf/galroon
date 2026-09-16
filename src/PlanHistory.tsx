import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ShieldCheck,ChevronRight} from 'lucide-react';
import {api,type Plan} from './api';
export function PlanHistory({plans,onOpen,updatedPlan}:{plans:Plan[];updatedPlan:Plan|null;onOpen:(plan:Plan)=>void}){
 const {t}=useTranslation();const [history,setHistory]=useState<Plan[]>(plans),[end,setEnd]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState('');
 const ids=new Set(plans.map(p=>p.id));const all=[...plans,...history.filter(p=>!ids.has(p.id))];
 useEffect(()=>{const ids=new Set(plans.map(p=>p.id));setHistory(old=>[...plans,...old.filter(p=>!ids.has(p.id))]);},[plans]);
 useEffect(()=>{if(updatedPlan)setHistory(old=>old.map(p=>p.id===updatedPlan.id?updatedPlan:updatedPlan.kind==='undo_organize'&&updatedPlan.state==='completed'&&p.id===updatedPlan.undo_of?{...p,reversed_by:updatedPlan.id}:p));},[updatedPlan]);
 const open=async(id:string)=>{setBusy(true);setError('');try{onOpen(await api<Plan>(`/plans/${id}`));}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}};
 const more=async()=>{if(!all.length)return;setBusy(true);setError('');try{const next=await api<Plan[]>(`/plans?before=${encodeURIComponent(all[all.length-1].id)}`);setHistory(old=>[...old,...next.filter(p=>!old.some(o=>o.id===p.id))]);setEnd(next.length<100);}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}};
 return <section><h2 className="section-heading">{t('plans')}</h2>{all.length===0?<p className="muted">{t('noPlans')}</p>:all.map(p=><button disabled={busy} className="plan-row" key={p.id} onClick={()=>void open(p.id)}><ShieldCheck size={20}/><span><strong>{t('planKind_'+p.kind)} · {p.items.length} {t('files')}</strong>{p.created&&<small>{new Date(p.created*1000).toLocaleString()}</small>}</span><span className={`pill ${p.state}`}>{t(p.reversed_by?'moveUndone':p.state)}</span><ChevronRight size={16}/></button>)}{!end&&all.length>=100&&<button disabled={busy} onClick={()=>void more()}>{t('olderPlans')}</button>}{error&&<p role="alert" className="error-text">{error}</p>}</section>;
}
