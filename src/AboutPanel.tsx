import {useTranslation} from 'react-i18next';
import {RefreshCw} from 'lucide-react';
import {APP_VERSION} from './version';
import type {useUpdates} from './useUpdates';
export function AboutPanel({updates,coreVersion,canWrite}:{updates:ReturnType<typeof useUpdates>;coreVersion?:string;canWrite:boolean}){
 const {t,i18n}=useTranslation();const {state,error,busy}=updates;
 return <div className="about-panel"><dl><dt>{t('updateCoreVersion')}</dt><dd>{coreVersion||state?.current||t('unknown')}</dd><dt>{t('updateClientVersion')}</dt><dd>{APP_VERSION}</dd>
  <dt>{t('updateLastChecked')}</dt><dd>{state?.last_checked!=null?new Date(state.last_checked*1000).toLocaleString(i18n.resolvedLanguage):t('updateNever')}</dd></dl>
  <p role="status">{t(busy||state?.checking?'updateChecking':!state?'updateStateUnavailable':state.update_available?'updateAvailable':state.last_error?'updateCheckFailed':state.last_checked===null?'updateNotChecked':'updateCurrent',{version:state?.latest?.version})}</p>
  {state?.last_error&&<p className="connection-error">{t('updateLastError')}: {state.last_error}</p>}{error&&<p role="alert" className="connection-error">{error}</p>}
  {state?.latest&&<><p>{state.latest.name}</p><p>{t(state.install_kind==='docker'?'updateDockerHint':'updateDownloadHint')}</p><a href={state.latest.url} target="_blank" rel="noreferrer">{t('updateOpenDownload')}</a></>}
  {state&&(canWrite?<section className="settings-row"><RefreshCw size={22}/><div><h2>{t('updateAutomatic')}</h2><p>{t('updatePrivacyHint')}</p></div><input type="checkbox" role="switch" checked={state.enabled} disabled={busy} aria-label={t('updateAutomatic')} onChange={e=>void updates.setEnabled(e.target.checked)}/></section>:<p>{t(state.enabled?'updateAutomaticEnabled':'updateAutomaticDisabled')}</p>)}
  {canWrite&&<button disabled={busy||state?.checking} onClick={()=>void updates.check()}>{t('updateCheckNow')}</button>}
 </div>;
}
