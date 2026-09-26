import {useTranslation} from 'react-i18next';
import type {Job} from './api';
const dimensions=['new_works','new_editions','new_resources','new_files','updated_files','unmatched_resources','needs_review','skipped','failed'] as const;
export function JobSummary({job}:{job:Job}){
 const {t}=useTranslation();
 if(!['scan','match'].includes(job.kind))return null;
 if(!job.summary)return <p className="muted">{t('jobSummaryMissing')}</p>;
 // Zero counts are recorded, but listing nine zeros hides the one number that changed.
 const changed=dimensions.filter(key=>job.summary![key]>0);
 return <section aria-label={t('jobSummaryTitle')} className="task-summary" title={t(job.kind==='scan'?'jobSummaryScanHint':'jobSummaryMatchHint')}>{changed.length?<dl>{changed.map(key=><div key={key} className={key==='failed'||key==='needs_review'?'attention':undefined}><dd>{job.summary![key].toLocaleString()}</dd><dt>{t('jobSummary_'+key)}</dt></div>)}</dl>:<p className="muted">{t('jobSummaryNoChanges')}</p>}</section>;
}
