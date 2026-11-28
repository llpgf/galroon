import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {bytes} from './api';
import {APP_VERSION} from './version';
import {shouldShowUpdate,skippedVersion,skipVersion,type UpdateState} from './updates';
import type {useUpdates} from './useUpdates';
export function UpdateBanner({state,coreVersion,canWrite=false,updates}:{state:UpdateState|null;coreVersion?:string;canWrite?:boolean;updates?:ReturnType<typeof useUpdates>}){
 const {t}=useTranslation();const [skipped,setSkipped]=useState(skippedVersion);const latest=shouldShowUpdate(state,skipped)?state?.latest:null;
 const mismatch=coreVersion&&APP_VERSION!==coreVersion;
 const applying=updates?.applying===true;
 const canApply=canWrite&&updates?.canApply===true;
 if(!latest&&!mismatch&&!applying)return null;
 const progress=updates?.progress;
 const pct=progress?.total&&progress.total>0?Math.min(100,Math.round(((progress.downloaded||0)/progress.total)*100)):undefined;
 const applyHintKey=updates?.canUpdateCore&&updates?.canUpdateDesktop?'updateBuiltInBothHint':updates?.canUpdateCore?'updateBuiltInCoreHint':updates?.canUpdateDesktop?'updateBuiltInDesktopHint':state?.install_kind==='docker'?'updateDockerHint':'updateDownloadHint';
 const applyButtonKey=updates?.canUpdateCore&&updates?.canUpdateDesktop?'updateNowBoth':updates?.canUpdateCore?'updateNowCore':updates?.canUpdateDesktop?'updateNowDesktop':'updateNow';
 return <section className="update-banner" aria-live="polite">
  {mismatch&&<p className="version-mismatch">{t('updateVersionMismatch',{client:APP_VERSION,core:coreVersion})}</p>}
  {latest&&<><strong>{t('updateAvailable',{version:latest.version})}</strong><p>{latest.name}</p><p>{t(canApply?applyHintKey:state?.install_kind==='docker'?'updateDockerHint':'updateDownloadHint')}</p>
   {latest.notes&&<details><summary>{t('updateReleaseNotes')}</summary><div className="update-notes">{latest.notes}</div></details>}
  </>}
  {applying&&progress&&<div className="update-progress" role="status">
   <p>{t(`updateStage_${progress.phase}`,{version:progress.targetVersion||latest?.version||''})}{progress.downloaded!==undefined&&progress.total?` · ${bytes(progress.downloaded)} / ${bytes(progress.total)} (${pct}%)`:''}</p>
   <progress value={pct} max={100}/>
  </div>}
  {updates?.error&&<p role="alert" className="connection-error">{updates.error}</p>}
  {state?.apply?.state==='failed'&&state.apply.error&&!updates?.error&&<p role="alert" className="connection-error">{state.apply.error}</p>}
  {(latest||canApply)&&<div className="update-actions">
   {canApply&&<button className="primary" disabled={applying||updates?.busy} onClick={()=>void updates?.apply(false)}>{t(applyButtonKey)}</button>}
   {canApply&&updates?.needsForceConfirm&&!applying&&<button onClick={()=>void updates.apply(true)}>{t('updatePauseTasksAndApply')}</button>}
   {latest&&<a href={latest.url} target="_blank" rel="noreferrer">{t('updateOpenDownload')}</a>}
   {latest&&<button disabled={applying} onClick={()=>{skipVersion(latest.version);setSkipped(latest.version);}}>{t('updateSkipVersion')}</button>}
  </div>}
 </section>;
}
