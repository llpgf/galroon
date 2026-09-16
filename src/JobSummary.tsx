import {useTranslation} from 'react-i18next';
import type {Job} from './api';
const dimensions=['new_works','new_editions','new_resources','new_files','updated_files','unmatched_resources','needs_review','skipped','failed'] as const;
export function JobSummary({job}:{job:Job}){
 const {t}=useTranslation();
 if(!['scan','match'].includes(job.kind))return null;
 if(!job.summary)return <p className="muted">{t('jobSummaryMissing')}</p>;
 return <section aria-label={t('jobSummaryTitle')} className="task-summary"><h3>{t('jobSummaryTitle')}</h3><dl>{dimensions.map(key=><div key={key}><dt>{t('jobSummary_'+key)}</dt><dd>{job.summary![key].toLocaleString()}</dd></div>)}</dl><p className="muted">{t(job.kind==='scan'?'jobSummaryScanHint':'jobSummaryMatchHint')}</p></section>;
}
