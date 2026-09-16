import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api} from './api';
export type TagReason={entity_id:string;name:string;kind:'person'|'character'|'company'};
export function RelatedTags({workId,reference=false,active,refreshKey,onOpen}:{workId:string;reference?:boolean;active:boolean;refreshKey:number;onOpen:(reason:TagReason)=>void}){
 const {t}=useTranslation();const [data,setData]=useState<{complete:boolean;tags:{id:string;name:string;reasons:TagReason[]}[]}|null>(null),[error,setError]=useState(''),[retry,setRetry]=useState(0);
 useEffect(()=>{if(!active)return;let live=true;setError('');setData(null);void api<NonNullable<typeof data>>(`/${reference?'vns':'works'}/${encodeURIComponent(workId)}/related-tags`).then(result=>{if(live)setData(result);}).catch(e=>{if(live)setError(String(e));});return()=>{live=false;};},[workId,reference,active,refreshKey,retry]);
 return <section className="explore-section"><h2>{t('relatedPersonalTags')}</h2>{error&&<p role="alert">{error} <button onClick={()=>setRetry(value=>value+1)}>{t('retry')}</button></p>}{data?.tags.map(tag=><div key={tag.id} className="related-tag-reason"><strong>{tag.name}</strong><div className="inline">{tag.reasons.map(reason=><button className="metadata-link" key={reason.entity_id} onClick={()=>onOpen(reason)}>{reason.name} <small>· {t('profile_'+reason.kind)}</small></button>)}</div></div>)}{data&&!data.complete&&<p>{t('relatedTagsIncomplete')}</p>}{data?.complete&&!data.tags.length&&<p>{t('relatedTagsEmpty')}</p>}</section>;
}
