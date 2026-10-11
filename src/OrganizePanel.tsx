import {useState} from 'react';import {useTranslation} from 'react-i18next';import {Check,Layers,Search,RefreshCw,MoreHorizontal,History,Link2,Combine,FolderInput,Wand2} from 'lucide-react';import {bytes,type Resource} from './api';import {coreText} from './coreText';import {useResourcePage} from './useResourcePage';
export function OrganizePanel({status,onStatus,generation,busy,onHistory,onMatch,onReferences,onGroup,onPreview}:{status:string;onStatus:(value:string)=>void;generation:number;busy:boolean;onHistory:(r:Resource)=>void;onMatch:(r:Resource)=>void;onReferences:(r:Resource)=>void;onGroup:(r:Resource)=>void;onPreview:(r:Resource)=>void}){
 const {t}=useTranslation(),[query,setQuery]=useState('');const page=useResourcePage(query,status,generation),rows=page.data?.items||[];
 // Menu items close their <details> before acting so the drawer opens on a clean table.
 const run=(event:React.MouseEvent<HTMLButtonElement>,fn:()=>void)=>{const menu=event.currentTarget.closest('details');if(menu)menu.open=false;fn();};
 return <section className="organize">
  <div className="organize-toolbar"><div className="tabs">{['all','unmatched','matched'].map(f=><button key={f} className={status===f?'selected':''} aria-pressed={status===f} onClick={()=>onStatus(f)}>{t(f==='all'?'allResources':f)}</button>)}</div>
   <label className="search organize-search"><Search size={16}/><input type="search" aria-label={t('resourceSearch')} placeholder={t('resourceSearch')} value={query} onChange={e=>setQuery(e.target.value)}/></label>
   {page.data&&<span className="muted organize-count">{t('resourceResultCount',{count:page.data.total})}</span>}
   <button className="icon-button" disabled={page.loading} aria-label={t('refreshView')} title={t('refreshView')} onClick={page.reload}><RefreshCw size={16}/></button>
  </div>
  {page.loading&&<p role="status" className="muted">{t('loading')}</p>}{page.error&&<div role="alert"><p>{page.error}</p><button onClick={page.reload}>{t('retry')}</button></div>}
  <div className="table-wrap"><table><thead><tr><th>{t('resourceColumn')}</th><th>{t('sizeColumn')}</th><th>{t('workColumn')}</th><th><span className="visually-hidden">{t('actionsColumn')}</span></th></tr></thead><tbody>{rows.map(r=><tr key={r.id} className={r.work_id?undefined:'unmatched-row'}><td><strong title={r.title||r.path}>{r.title||r.path}</strong>{r.title&&r.path!==r.title&&<small className="path" title={r.path}>{r.path}</small>}{!r.work_id&&r.auto_match_reason&&<small className="match-review-reason">{t('matchWhyUnmatched')}: {coreText(t,r.auto_match_reason)}</small>}</td><td className="size-cell">{bytes(r.bytes)}<small>{r.files} {t('files')} · {t('resourceKind_'+r.kind,{defaultValue:r.kind})}</small></td><td>{r.work_id?<span className="matched-text"><Check size={14}/>{r.primary_work_title||r.work_id}</span>:<span className="pill partial">{t('unmatched')}</span>}</td><td><div className="row-actions">
   {r.work_id?<button disabled={busy} onClick={()=>onPreview(r)}><FolderInput size={15}/>{t('preview')}</button>:<button className="primary" onClick={()=>onMatch(r)}><Wand2 size={15}/>{t('match')}</button>}
   <details className="toolbar-menu"><summary aria-label={t('moreOptions')} title={t('moreOptions')}><MoreHorizontal size={18}/></summary><div className="toolbar-menu-panel">
    {r.work_id&&<button onClick={e=>run(e,()=>onMatch(r))}><Wand2 size={15}/>{t('match')}</button>}
    <button onClick={e=>run(e,()=>onHistory(r))}><History size={15}/>{t('matchingHistory')}</button>
    <button onClick={e=>run(e,()=>onReferences(r))}><Link2 size={15}/>{t('editReferences')}</button>
    <button onClick={e=>run(e,()=>onGroup(r))}><Combine size={15}/>{t('resourceGrouping')}</button>
   </div></details>
  </div></td></tr>)}</tbody></table>{page.data&&rows.length===0&&<div className="empty">{status==='unmatched'&&!query?<><Check size={30}/><p>{t('organizeAllMatched')}</p></>:<><Layers size={30}/><p>{t('noResults')}</p></>}</div>}</div>
  {(page.pageIndex>0||page.data?.next)&&<nav className="resource-pagination" aria-label={t('organize')}><button disabled={page.loading||page.pageIndex===0} onClick={page.previous}>{t('previousPage')}</button><span>{page.pageIndex+1}</span><button disabled={page.loading||!page.data?.next} onClick={page.next}>{t('nextPage')}</button></nav>}
 </section>;
}
