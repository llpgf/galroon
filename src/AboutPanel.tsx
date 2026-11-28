import {useTranslation} from 'react-i18next';
import {AlertTriangle,RefreshCw} from 'lucide-react';
import {bytes} from './api';
import {APP_VERSION} from './version';
import type {useUpdates} from './useUpdates';
export function AboutPanel({updates,coreVersion,canWrite}:{updates:ReturnType<typeof useUpdates>;coreVersion?:string;canWrite:boolean}){
 const {t,i18n}=useTranslation();const {state,error,busy}=updates;
 const applying=updates.applying===true;
 const canApply=canWrite&&updates.canApply===true;
 const progress=updates.progress;
 const pct=progress?.total&&progress.total>0?Math.min(100,Math.round(((progress.downloaded||0)/progress.total)*100)):undefined;
 const bothSteps=progress?.totalSteps===2||!!(updates.canUpdateCore&&updates.canUpdateDesktop);
 const stepLabel=bothSteps&&progress?t((progress.step===2||progress.phase.startsWith('desktop_'))?'updateStep2Of2':'updateStep1Of2'):'';
 const applyButtonKey=bothSteps?'updateNowBoth':updates.canUpdateCore?'updateNowCore':updates.canUpdateDesktop?'updateNowDesktop':'updateNow';
 const alertText=updates.needsForceConfirm?t('updateBlockedActiveTasks'):error||(state?.apply?.state==='failed'&&state.apply.error?state.apply.error:'');
 return <div className="about-panel"><dl><dt>{t('updateCoreVersion')}</dt><dd>{coreVersion||state?.current||t('unknown')}</dd><dt>{t('updateClientVersion')}</dt><dd>{APP_VERSION}</dd>
  <dt>{t('updateLastChecked')}</dt><dd>{state?.last_checked!=null?new Date(state.last_checked*1000).toLocaleString(i18n.resolvedLanguage):t('updateNever')}</dd></dl>
  <p role="status">{t(busy||state?.checking?'updateChecking':!state?'updateStateUnavailable':state.update_available?'updateAvailable':state.last_error?'updateCheckFailed':state.last_checked===null?'updateNotChecked':'updateCurrent',{version:state?.latest?.version})}</p>
  {state?.last_error&&<div className="update-alert danger"><AlertTriangle size={16}/><span>{t('updateLastError')}: {state.last_error}</span></div>}
  {alertText&&<div role="alert" className={`update-alert ${updates.needsForceConfirm?'warn':'danger'}`}><AlertTriangle size={16}/><span>{alertText}</span></div>}
  {applying&&progress&&<div className="update-progress-card" role="status">
   <div className="update-progress-head"><RefreshCw size={15} className="spin"/><span>{stepLabel?`${stepLabel} · `:''}{t(`updateStage_${progress.phase}`,{version:progress.targetVersion||state?.latest?.version||''})}{progress.downloaded!==undefined&&progress.total?` · ${bytes(progress.downloaded)} / ${bytes(progress.total)} (${pct}%)`:''}</span></div>
   <progress value={pct} max={100}/>
  </div>}
  {state?.latest&&<><p>{state.latest.name}</p><p>{t(canApply?(bothSteps?'updateBuiltInBothHint':updates.canUpdateCore?'updateBuiltInCoreHint':'updateBuiltInDesktopHint'):state.install_kind==='docker'?'updateDockerHint':'updateDownloadHint')}</p></>}
  <div className="update-actions">
   {canApply&&(updates.needsForceConfirm&&!applying
    ?<button className="primary" disabled={busy} onClick={()=>void updates.apply(true)}>{t('updatePauseTasksAndApply')}</button>
    :<button className="primary" disabled={applying||busy} onClick={()=>void updates.apply(false)}>{applying&&<RefreshCw size={14} className="spin"/>}{t(applying?'updateInProgress':applyButtonKey)}</button>)}
   {state?.latest&&<a href={state.latest.url} target="_blank" rel="noreferrer">{t('updateOpenDownload')}</a>}
   {canWrite&&<button disabled={busy||applying||state?.checking} onClick={()=>void updates.check()}>{t('updateCheckNow')}</button>}
  </div>
  {state&&(canWrite?<section className="settings-row"><RefreshCw size={22}/><div><h2>{t('updateAutomatic')}</h2><p>{t('updatePrivacyHint')}</p></div><input type="checkbox" role="switch" checked={state.enabled} disabled={busy||applying} aria-label={t('updateAutomatic')} onChange={e=>void updates.setEnabled(e.target.checked)}/></section>:<p>{t(state.enabled?'updateAutomaticEnabled':'updateAutomaticDisabled')}</p>)}
 </div>;
}
