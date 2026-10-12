import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {APP_VERSION} from './version';
import {shouldShowUpdate,skippedVersion,skipVersion,type UpdateState} from './updates';
export function UpdateBanner({state,coreVersion}:{state:UpdateState|null;coreVersion?:string}){
 const {t}=useTranslation();const [skipped,setSkipped]=useState(skippedVersion);const latest=shouldShowUpdate(state,skipped)?state?.latest:null;
 const mismatch=coreVersion&&APP_VERSION!==coreVersion;if(!latest&&!mismatch)return null;
 return <section className="update-banner" aria-live="polite">
  {mismatch&&<p className="version-mismatch">{t('updateVersionMismatch',{client:APP_VERSION,core:coreVersion})}</p>}
  {latest&&<><strong>{t('updateAvailable',{version:latest.version})}</strong><p>{latest.name}</p><p>{t(state?.install_kind==='docker'?'updateDockerHint':'updateDownloadHint')}</p>
   {latest.notes&&<details><summary>{t('updateReleaseNotes')}</summary><div className="update-notes">{latest.notes}</div></details>}
   <div className="update-actions"><a href={latest.url} target="_blank" rel="noreferrer">{t('updateOpenDownload')}</a><button onClick={()=>{skipVersion(latest.version);setSkipped(latest.version);}}>{t('updateSkipVersion')}</button></div>
  </>}
 </section>;
}
