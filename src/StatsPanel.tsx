import {useMemo} from 'react';
import {useTranslation} from 'react-i18next';
import {useCollectionPage,type CollectionQuery} from './useCollectionPage';
import {emptyFilters,type Filters} from './library';

type TitleMode='display'|'original';
type Open=(status?:string,filters?:Partial<Filters>)=>void;
const STATUSES=['none','playing','completed','backlog','on_hold','dropped'] as const;
const AVAILABILITY=['available','offline','missing','unverified','no_resources','unknown'] as const;
type Row={key:string;label:string;count:number;open:()=>void};

/** Ranks facet options by their whole-collection counts. */
function ranked(options:{value:string;label:string}[]|undefined,counts:Record<string,number>|undefined,limit:number,open:(value:string)=>Row['open'],label=(s:string)=>s):Row[]{
 return (options||[]).map(option=>({key:option.value,label:label(option.label),count:counts?.[option.value]||0,open:open(option.value)})).filter(row=>row.count).sort((a,b)=>b.count-a.count).slice(0,limit);
}

/**
 * Statistics: the composition of the whole collection. Every number comes from the collection page's
 * metadata, so it matches what the collection shows, and every row opens the collection filtered to it.
 */
export function StatsPanel({generation,titleMode,onCollection}:{generation:number;titleMode:TitleMode;onCollection:Open}){
 const {t,i18n}=useTranslation();
 const request:CollectionQuery={filters:emptyFilters,query:'',status:'all',sort:'added',titleMode,locale:i18n.language};
 const page=useCollectionPage(request,generation);
 const data=page.data;
 const total=data?.total??0;
 const pct=(count:number)=>total?Math.round(count/total*100)+'%':'0%';
 const languageName=useMemo(()=>{try{const names=new Intl.DisplayNames([i18n.language],{type:'language'});return (code:string)=>{try{return names.of(code)||code;}catch{return code;}};}catch{return (code:string)=>code;}},[i18n.language]);

 const view=useMemo(()=>{
  if(!data)return null;
  const statuses=STATUSES.map(key=>({key,label:t(key),count:data.status_counts[key]||0,open:()=>onCollection(key)}));
  const availability=AVAILABILITY.map(key=>({key,label:t('availability_'+key),count:data.availability_counts?.[key]||0,open:()=>onCollection(undefined,{availability:key})})).filter(row=>row.count);
  const years=(data.options.year||[]).map(option=>({value:option.value,year:Number(option.label),count:data.year_counts?.[option.value]||0})).filter(entry=>entry.count&&Number.isFinite(entry.year));
  let span:{value:string;year:number;count:number}[]=[];
  if(years.length){const lo=Math.min(...years.map(y=>y.year)),hi=Math.max(...years.map(y=>y.year));const byYear=new Map(years.map(y=>[y.year,y]));for(let year=lo;year<=hi;year++)span.push(byYear.get(year)||{value:'',year,count:0});}
  const peak=span.reduce<{year:number;count:number}|null>((best,y)=>!best||y.count>best.count?y:best,null);
  const ratings=Array.from({length:10},(_,i)=>({score:i+1,count:data.rating_counts?.[String(i+1)]||0}));
  return {
   statuses,availability,span,peak,ratings,rated:ratings.reduce((sum,r)=>sum+r.count,0),
   studios:ranked(data.options.studio,data.studio_counts,10,value=>()=>onCollection(undefined,{studio:value})),
   tags:ranked(data.options.tag,data.tag_counts,10,value=>()=>onCollection(undefined,{tags:[value]})),
   languages:ranked(data.options.language,data.language_counts,8,value=>()=>onCollection(undefined,{language:value}),languageName),
  };
 },[data,t,onCollection,languageName]);

 return <div className="stats">
  <div className="page-heading"><div><h1>{t('statsTitle')}</h1><p>{data?t('statsSubtitle',{count:total}):t('loading')}</p></div></div>
  {view&&total===0&&<p className="stats-empty">{t('statsEmpty')}</p>}
  {view&&total>0&&<>
   <div className="stats-kpis">
    <button className="stats-kpi" onClick={()=>onCollection()}><span>{t('statsWorks')}</span><strong>{total}</strong><small>{t('statsAllWorks')}</small></button>
    <button className="stats-kpi" onClick={()=>onCollection(undefined,{availability:'available'})}><span>{t('statsReady')}</span><strong>{data!.availability_counts?.available||0}</strong><small>{t('statsShare',{pct:pct(data!.availability_counts?.available||0)})}</small></button>
    <button className="stats-kpi" onClick={()=>onCollection('completed')}><span>{t('completed')}</span><strong>{data!.status_counts.completed||0}</strong><small>{t('statsShare',{pct:pct(data!.status_counts.completed||0)})}</small></button>
    <button className="stats-kpi" onClick={()=>onCollection('favorite')}><span>{t('favorite')}</span><strong>{data!.favorite_count||0}</strong><small>{t('statsShare',{pct:pct(data!.favorite_count||0)})}</small></button>
   </div>

   <div className="stats-pair">
    <section className="stats-card" aria-labelledby="stats-status">
     <h2 id="stats-status">{t('statsStatus')}</h2>
     <div className="home-bar stats-split" aria-hidden="true">{view.statuses.filter(row=>row.count).map(row=><span key={row.key} className={'seg-'+row.key} style={{flex:row.count}}/>)}</div>
     <ul className="stats-legend">{view.statuses.map(row=><li key={row.key}><button className="stats-row" onClick={row.open}><span className={'dot seg-'+row.key}/>{row.label}<span>{row.count} · {pct(row.count)}</span></button></li>)}</ul>
    </section>
    <section className="stats-card" aria-labelledby="stats-availability">
     <h2 id="stats-availability">{t('statsAvailability')}</h2>
     <Bars rows={view.availability}/>
     <p className="stats-note">{t('statsAvailabilityHint')}</p>
    </section>
   </div>

   {view.span.length>0&&<section className="stats-card" aria-labelledby="stats-years">
    <header><h2 id="stats-years">{t('statsYears')}</h2>{view.peak&&<span>{t('statsPeak',{year:view.peak.year,count:view.peak.count})}</span>}</header>
    <div className="stats-columns" style={{gridTemplateColumns:`repeat(${view.span.length},minmax(0,1fr))`}}>
     {view.span.map(entry=><button key={entry.year} className={entry.year===view.peak?.year?'peak':undefined} disabled={!entry.count} title={t('statsYearBar',{year:entry.year,count:entry.count})} aria-label={t('statsYearBar',{year:entry.year,count:entry.count})} onClick={()=>onCollection(undefined,{year:entry.value})}><span style={{height:Math.max(entry.count?4:0,entry.count/(view.peak?.count||1)*100)+'%'}}/></button>)}
    </div>
    <div className="stats-axis" aria-hidden="true" style={{gridTemplateColumns:`repeat(${view.span.length},minmax(0,1fr))`}}>{view.span.map(entry=><span key={entry.year}>{entry.year%5===0||entry===view.span.at(-1)||entry===view.span[0]?String(entry.year).slice(2):''}</span>)}</div>
   </section>}

   <div className="stats-pair">
    <section className="stats-card" aria-labelledby="stats-studios"><h2 id="stats-studios">{t('statsStudios')}</h2><Bars rows={view.studios} numbered/></section>
    <section className="stats-card" aria-labelledby="stats-tags"><h2 id="stats-tags">{t('statsTags')}</h2><Bars rows={view.tags} numbered/></section>
   </div>

   <div className="stats-pair">
    <section className="stats-card" aria-labelledby="stats-languages"><h2 id="stats-languages">{t('statsLanguages')}</h2><Bars rows={view.languages}/><p className="stats-note">{t('statsLanguagesHint')}</p></section>
    <section className="stats-card" aria-labelledby="stats-ratings">
     <header><h2 id="stats-ratings">{t('statsRatings')}</h2><span>{t('statsRated',{count:view.rated})}</span></header>
     {view.rated?<>
      <div className="stats-columns stats-ratings" role="img" aria-label={view.ratings.map(r=>t('statsRatingBar',{score:r.score,count:r.count})).join('，')}>{view.ratings.map(r=><span key={r.score} className={r.count===Math.max(...view.ratings.map(x=>x.count))?'peak':undefined}><span style={{height:Math.max(r.count?4:0,r.count/Math.max(...view.ratings.map(x=>x.count))*100)+'%'}}/></span>)}</div>
      <div className="stats-axis" aria-hidden="true" style={{gridTemplateColumns:'repeat(10,minmax(0,1fr))'}}>{view.ratings.map(r=><span key={r.score}>{r.score}</span>)}</div>
     </>:<p className="stats-note">{t('statsNoRatings')}</p>}
    </section>
   </div>
  </>}
 </div>;
}

function Bars({rows,numbered=false}:{rows:Row[];numbered?:boolean}){
 const top=rows[0]?.count||1;
 return <ol className={'stats-bars'+(numbered?' numbered':'')}>{rows.map((row,index)=><li key={row.key}><button className="stats-bar" onClick={row.open}>
  {numbered&&<span className="stats-rank">{index+1}</span>}
  <span className="stats-label">{row.label}</span>
  <span className="home-meter" aria-hidden="true"><span style={{width:row.count/top*100+'%'}}/></span>
  <span className="stats-count">{row.count}</span>
 </button></li>)}</ol>;
}
