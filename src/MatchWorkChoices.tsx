import {useTranslation} from 'react-i18next';
import {emptyFilters} from './library';
import {useCollectionPage} from './useCollectionPage';
export function MatchWorkChoices({query,generation,busy,onChoose}:{query:string;generation:number;busy:boolean;onChoose:(id:string)=>void}){
 const {t,i18n}=useTranslation();
 const page=useCollectionPage({filters:emptyFilters,query,status:'all',sort:'title',titleMode:'display',locale:i18n.language},generation);
 return <section className="match-work-choices"><h3>{t('existing')}</h3>{page.loading&&<p role="status">{t('loading')}</p>}{page.error&&<div role="alert"><p>{page.error}</p><button disabled={busy} onClick={page.reload}>{t('retry')}</button></div>}{page.data?.items.map(w=><button className="candidate" disabled={busy} key={w.id} onClick={()=>onChoose(w.id)}><span>{w.title}<small>{w.original_title}</small></span></button>)}{page.data&&page.data.items.length===0&&<p>{t('noResults')}</p>}<nav className="inline" aria-label={t('existing')}><button disabled={busy||page.loading||page.pageIndex===0} onClick={page.previous}>{t('previousPage')}</button><span>{page.pageIndex+1}</span><button disabled={busy||page.loading||!page.data?.next} onClick={page.next}>{t('nextPage')}</button></nav></section>;
}
