import {useTranslation} from 'react-i18next';
import {RefreshCw} from 'lucide-react';
import {bytes} from './api';
import {APP_VERSION} from './version';
import type {useUpdates} from './useUpdates';
export function AboutPanel({updates,coreVersion,canWrite}:{updates:ReturnType<typeof useUpdates>;coreVersion?:string;canWrite:boolean}){
 const {t,i18n}=useTranslation();const {state,error,busy}=updates;
 const applying=updates.applying===true;
 const canApply=canWrite&&updates.canApply===true;
 const progress=updates.progress;
 const pct=progress?.total&&progress.total>0?Math.min(100,Math.round(((progress.downloaded||0)/progress.total)*100)):undefined;
 const applyButtonKey=updates.canUpdateCore&&updates.canUpdateDesktop?'updateNowBoth':updates.canUpdateCore?'updateNowCore':updates.canUpdateDesktop?'updateNowDesktop':'updateNow';
 return <div className="about-panel"><dl><dt>{t('updateCoreVersion')}</dt><dd>{coreVersion||state?.current||t('unknown')}</dd><dt>{t('updateClientVersion')}</dt><dd>{APP_VERSION}</dd>
  <dt>{t('updateLastChecked')}</dt><dd>{state?.last_checked!=null?new Date(state.last_checked*1000).toLocaleString(i18n.resolvedLanguage):t('updateNever')}</dd></dl>
  <p role="status">{t(busy||state?.checking?'updateChecking':!state?'updateStateUnavailable':state.update_available?'updateAvailable':state.last_error?'updateCheckFailed':state.last_checked===null?'updateNotChecked':'updateCurrent',{version:state?.latest?.version})}</p>
  {state?.last_error&&<p className="connection-error">{t('updateLastError')}: {state.last_error}</p>}
  {state?.apply?.state==='failed'&&state.apply.error&&!error&&<p role="alert" className="connection-error">{state.apply.error}</p>}
  {error&&<p role="alert" className="connection-error">{error}</p>}
  {applying&&progress&&<div className="update-progress" role="status">
   <p>{t(`updateStage_${progress.phase}`,{version:progress.targetVersion||state?.latest?.version||''})}{progress.downloaded!==undefined&&progress.total?` · ${bytes(progress.downloaded)} / ${bytes(progress.total)} (${pct}%)`:''}</p>
   <progress value={pct} max={100}/>
  </div>}
  {state?.latest&&<><p>{state.latest.name}</p><p>{t(canApply?(updates.canUpdateCore&&updates.canUpdateDesktop?'updateBuiltInBothHint':updates.canUpdateCore?'updateBuiltInCoreHint':'updateBuiltInDesktopHint'):state.install_kind==='docker'?'updateDockerHint':'updateDownloadHint')}</p></>}
  <div className="update-actions">
   {canApply&&<button className="primary" disabled={applying||busy} onClick={()=>void updates.apply(false)}>{t(applyButtonKey)}</button>}
   {canApply&&updates.needsForceConfirm&&!applying&&<button onClick={()=>void updates.apply(true)}>{t('updatePauseTasksAndApply')}</button>}
   {state?.latest&&<a href={state.latest.url} target="_blank" rel="noreferrer">{t('updateOpenDownload')}</a>}
   {canWrite&&<button disabled={busy||applying||state?.checking} onClick={()=>void updates.check()}>{t('updateCheckNow')}</button>}
  </div>
  {state&&(canWrite?<section className="settings-row"><RefreshCw size={22}/><div><h2>{t('updateAutomatic')}</h2><p>{t('updatePrivacyHint')}</p></div><input type="checkbox" role="switch" checked={state.enabled} disabled={busy||applying} aria-label={t('updateAutomatic')} onChange={e=>void updates.setEnabled(e.target.checked)}/></section>:<p>{t(state.enabled?'updateAutomaticEnabled':'updateAutomaticDisabled')}</p>)}
 </div>;
}
