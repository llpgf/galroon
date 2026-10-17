import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ChevronLeft,ChevronRight} from 'lucide-react';
import {api,type Work} from './api';
import {CachedImage} from './CachedImage';
import type {DiscoveryWork} from './discovery';

export type Release={release_id:string;vn_id:string;released:string;title:string;original_title:string;producers:string[];image:{url:string}|null;work_id?:string;status?:string};
/** The reference page's first paint for a release outside the collection; it then loads the full VNDB entry. */
export function releaseReference(release:Release):DiscoveryWork{
 const title=release.title||release.original_title;
 return {id:release.vn_id,title,alttitle:release.original_title&&release.original_title!==title?release.original_title:undefined,released:release.released,image:release.image?{url:release.image.url}:undefined};
}
type Calendar={month:string;current_month:string;window:number;fetched_at:number;expires_at:number;releases:Release[]};

/** Months as `year * 12 + month - 1`, the same arithmetic Core uses for its window. */
const index=(month:string)=>{const [y,m]=month.split('-').map(Number);return y*12+m-1;};
const label=(value:number)=>`${Math.floor(value/12)}-${String(value%12+1).padStart(2,'0')}`;
function localMonth(){const now=new Date();return label(now.getFullYear()*12+now.getMonth());}

/**
 * Release calendar on the home page: VNDB releases for this month and six months either side.
 * Core caches each month for a week in a separate file; nothing here is added to the collection.
 */
export function ReleaseCalendar({safe,titleMode,onSelect,onReference}:{safe:boolean;titleMode:'display'|'original';onSelect:(w:Work)=>void;onReference:(work:DiscoveryWork)=>void}){
 const {t,i18n}=useTranslation();
 const [month,setMonth]=useState(localMonth);
 const [data,setData]=useState<Calendar|null>(null),[error,setError]=useState(''),[loading,setLoading]=useState(true);
 const seen=useRef(new Map<string,Calendar>());
 useEffect(()=>{
  let live=true;const hit=seen.current.get(month);
  if(hit){setData(hit);setError('');setLoading(false);return;}
  setLoading(true);setError('');
  api<Calendar>('/release-calendar?'+new URLSearchParams({month})).then(result=>{if(!live)return;seen.current.set(month,result);setData(result);}).catch(e=>{if(live)setError(e instanceof Error?e.message:String(e));}).finally(()=>{if(live)setLoading(false);});
  return ()=>{live=false;};
 },[month]);

 // Core decides which month is "now" and how far the window reaches.
 const current=data?.current_month||localMonth(),reach=data?.window??6;
 const offset=index(month)-index(current);
 const go=(step:number)=>setMonth(label(index(month)+step));
 const releases=data?.month===month?data.releases:[];
 const owned=releases.filter(r=>r.work_id).length;
 const [year,monthNumber]=month.split('-').map(Number);
 const heading=new Intl.DateTimeFormat(i18n.language,{year:'numeric',month:'long'}).format(new Date(year,monthNumber-1,1));
 const weekday=new Intl.DateTimeFormat(i18n.language,{weekday:'short'});
 const open=async(id:string)=>{try{onSelect(await api<Work>(`/works/${encodeURIComponent(id)}`));}catch(e){setError(e instanceof Error?e.message:String(e));}};

 return <section className="home-tile home-calendar" aria-labelledby="home-calendar-title" aria-busy={loading}>
  <header>
   <div><h2 id="home-calendar-title">{t('homeCalendar')}</h2>{!loading&&!error&&<span>{owned?t('homeCalendarSummaryOwned',{count:releases.length,owned}):t('homeCalendarSummary',{count:releases.length})}</span>}</div>
   <div className="home-calendar-nav">
    <button className="home-round" aria-label={t('homeCalendarPrevious')} disabled={offset<=-reach} onClick={()=>go(-1)}><ChevronLeft size={16}/></button>
    <strong aria-live="polite">{heading}</strong>
    <button className="home-round" aria-label={t('homeCalendarNext')} disabled={offset>=reach} onClick={()=>go(1)}><ChevronRight size={16}/></button>
    <button className="home-calendar-today" disabled={offset===0} onClick={()=>setMonth(current)}>{t('homeCalendarToday')}</button>
   </div>
  </header>
  <div className="home-calendar-ticks" aria-hidden="true">{Array.from({length:reach*2+1},(_,i)=>i-reach).map(step=><span key={step} className={step===offset?'on':step===0?'now':undefined}/>)}</div>
  {error?<p className="home-calendar-empty" role="alert">{t('homeCalendarError',{message:error})}</p>
   :loading?<p className="home-calendar-empty">{t('loading')}</p>
   :releases.length===0?<p className="home-calendar-empty">{t('homeCalendarNone')}</p>
   :<ol className="home-calendar-list">{releases.map(release=>{
    const exact=/^\d{4}-\d{2}-\d{2}$/.test(release.released),date=new Date(release.released+'T00:00:00');const title=titleMode==='original'?release.original_title||release.title:release.title||release.original_title;
    const body=<>
     <span className="home-calendar-day">{exact?<><strong>{date.getDate()}</strong><small>{weekday.format(date)}</small></>:<small>{t('homeCalendarUndated')}</small>}</span>
     <span className="home-calendar-cover cover">{release.image&&!safe&&<CachedImage source={release.image.url}/>}</span>
     <span className="home-calendar-text"><strong>{title}</strong><span>{release.producers.join('、')}</span></span>
     {release.work_id&&<span className={'home-calendar-badge status-'+release.status}>{t(release.status==='backlog'?'homeCalendarWanted':'homeCalendarOwned')}</span>}
    </>;
    return <li key={release.vn_id}>{release.work_id?<button className="home-calendar-row" onClick={()=>void open(release.work_id!)}>{body}</button>:<button className="home-calendar-row" onClick={()=>onReference(releaseReference(release))}>{body}</button>}</li>;
   })}</ol>}
  <p className="home-calendar-note">{t('homeCalendarNote')}</p>
 </section>;
}
