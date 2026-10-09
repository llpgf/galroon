import {useEffect,useMemo,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ChevronLeft,ChevronRight,ChevronRight as More,Shuffle,Check,Heart} from 'lucide-react';
import type {Work} from './api';
import {CachedImage} from './CachedImage';
import {useCollectionPage,type CollectionQuery} from './useCollectionPage';
import {emptyFilters,titleFor,coverTone,tagNames,type Filters} from './library';

type TitleMode='display'|'original';
const SLIDE_MS=7000;
const PICK_SCOPES=['backlog','on_hold'] as const;

/** Artwork or the tinted title placeholder, shared by every cover on the home page. */
function Cover({work,safe,className=''}:{work:Work;safe:boolean;className?:string}){
 const fallback=<span className="cover-title">{work.original_title||work.title}</span>;
 return <div className={`cover cover-${coverTone(work.id)} ${className}`}>{work.cover&&!safe?<CachedImage source={work.cover} fallback={fallback}/>:fallback}{work.favorite&&<Heart className="cover-heart" size={16} fill="currentColor"/>}</div>;
}

function useWorks(filters:Filters,status:string,sort:string,titleMode:TitleMode,generation:number){
 const {i18n}=useTranslation();
 const request:CollectionQuery={filters,query:'',status,sort,titleMode,locale:i18n.language};
 return useCollectionPage(request,generation);
}

/**
 * Home: a showcase of the organised collection. A featured carousel (works in progress, then recent
 * additions), a "what to play next" picker over the backlog, an overview and shelves. Everything is read
 * from the same paged collection API the collection page uses; nothing here changes the collection.
 */
export function HomePanel({generation,safe,titleMode,unmatched,activeTasks,onSelect,onCollection,onOrganize,onTasks}:{generation:number;safe:boolean;titleMode:TitleMode;unmatched:number;activeTasks:number;onSelect:(w:Work)=>void;onCollection:(status?:string)=>void;onOrganize:()=>void;onTasks:()=>void}){
 const {t}=useTranslation();
 const playing=useWorks(emptyFilters,'playing','added',titleMode,generation);
 const recent=useWorks(emptyFilters,'all','added',titleMode,generation);
 const favorites=useWorks({...emptyFilters,favorites:true},'all','added',titleMode,generation);

 const slides=useMemo(()=>{const seen=new Set<string>();const out:{work:Work;reason:string}[]=[];
  for(const work of playing.data?.items.slice(0,3)||[]){seen.add(work.id);out.push({work,reason:t('homeContinue')});}
  for(const work of recent.data?.items||[]){if(out.length>=5)break;if(!seen.has(work.id)){seen.add(work.id);out.push({work,reason:t('homeRecent')});}}
  return out;},[playing.data,recent.data,t]);
 const [slide,setSlide]=useState(0),[paused,setPaused]=useState(false);
 const current=slides.length?slides[slide%slides.length]:null;
 useEffect(()=>{if(slides.length<2||paused||window.matchMedia('(prefers-reduced-motion: reduce)').matches)return;const id=setInterval(()=>setSlide(n=>(n+1)%slides.length),SLIDE_MS);return()=>clearInterval(id);},[slides.length,paused,slide]);
 const go=(n:number)=>setSlide((n+slides.length)%slides.length);

 const [scope,setScope]=useState<typeof PICK_SCOPES[number]>('backlog'),[ready,setReady]=useState(true),[tag,setTag]=useState(''),[pick,setPick]=useState(0);
 const pickFilters=useMemo<Filters>(()=>({...emptyFilters,availability:ready?'available':'',tags:tag?[tag]:[]}),[ready,tag]);
 const pool=useWorks(pickFilters,scope,'added',titleMode,generation);
 const tagMeta=useWorks(emptyFilters,scope,'added',titleMode,generation);
 const tagChoices=useMemo(()=>{const counts=tagMeta.data?.tag_counts||{};return (tagMeta.data?.options.tag||[]).filter(option=>counts[option.value]).sort((a,b)=>(counts[b.value]||0)-(counts[a.value]||0)).slice(0,8);},[tagMeta.data]);
 const items=pool.data?.items||[];
 const chosen=items.length?items[pick%items.length]:null;
 const others=items.map((work,index)=>({work,index})).filter(entry=>entry.work.id!==chosen?.id).slice(0,4);
 const shuffle=()=>{if(items.length<2)return;let next=pick%items.length;while(next===pick%items.length)next=Math.floor(Math.random()*items.length);setPick(next);};
 useEffect(()=>setPick(0),[scope,ready,tag]);

 const counts=recent.data?.status_counts||{},total=recent.data?.total??0;
 const segments=(['playing','completed','on_hold','backlog'] as const).map(key=>({key,count:counts[key]||0}));

 return <div className="home">
  <div className="home-top">
   <h1>{t('homeGreeting')}</h1>
   <div className="home-alerts">
    {unmatched>0&&<button className="home-alert warn" onClick={onOrganize}><span className="dot"/>{t('homeNeedsMatching',{count:unmatched})}</button>}
    {activeTasks>0&&<button className="home-alert live" onClick={onTasks}><span className="dot"/>{t('homeTasksRunning',{count:activeTasks})}</button>}
   </div>
  </div>

  {current&&<section className="home-hero" aria-roledescription="carousel" aria-label={t('homeFeatured')} onMouseEnter={()=>setPaused(true)} onMouseLeave={()=>setPaused(false)} onFocus={()=>setPaused(true)} onBlur={()=>setPaused(false)}>
   <div className="home-hero-backdrop" aria-hidden="true">{current.work.cover&&!safe&&<CachedImage key={current.work.id} source={current.work.cover}/>}</div>
   <div className="home-hero-copy" key={current.work.id}>
    <span className="home-hero-reason">{current.reason}</span>
    <h2>{titleFor(current.work,titleMode)}</h2>
    <p>{[current.work.developer,current.work.released?.slice(0,4)].filter(Boolean).join(' · ')}</p>
    <div className="home-hero-tags">{tagNames(current.work.tags).slice(0,3).map(name=><span key={name}>{name}</span>)}</div>
    <div className="home-hero-actions"><button className="home-hero-primary" onClick={()=>onSelect(current.work)}>{t('homeViewWork')}<ChevronRight size={16}/></button></div>
   </div>
   <button className="home-hero-art" key={'art-'+current.work.id} aria-label={t('openWorkNamed',{name:titleFor(current.work,titleMode)})} onClick={()=>onSelect(current.work)}><Cover work={current.work} safe={safe}/></button>
   {slides.length>1&&<div className="home-hero-controls">
    <div className="home-dots">{slides.map((entry,index)=><button key={entry.work.id} aria-label={t('homeSlide',{title:titleFor(entry.work,titleMode)})} aria-current={index===slide%slides.length?'true':undefined} onClick={()=>go(index)}/>)}</div>
    <button className="home-round" aria-label={t('homePrevious')} onClick={()=>go(slide-1)}><ChevronLeft size={16}/></button>
    <button className="home-round" aria-label={t('homeNext')} onClick={()=>go(slide+1)}><ChevronRight size={16}/></button>
   </div>}
  </section>}

  <div className="home-grid">
   <section className="home-card home-picker" aria-labelledby="home-pick-title">
    <header>
     <div><h2 id="home-pick-title">{t('homePickTitle')}</h2><p>{pool.data?t('homePickCount',{count:pool.data.filtered}):t('loading')}</p></div>
     <button className="primary" disabled={items.length<2} onClick={shuffle}><Shuffle size={16}/>{t('homePickShuffle')}</button>
    </header>
    <div className="home-picker-filters">
     <div className="tabs" role="radiogroup" aria-label={t('focusStatus')}>{PICK_SCOPES.map(value=><button key={value} role="radio" aria-checked={scope===value} aria-pressed={scope===value} onClick={()=>setScope(value)}>{t(value)}<span className="home-count">{counts[value]||0}</span></button>)}</div>
     <button className="home-toggle" role="switch" aria-checked={ready} onClick={()=>setReady(!ready)}><Check size={14}/>{t('homePickReady')}</button>
    </div>
    {tagChoices.length>0&&<div className="home-chips">
     <button aria-pressed={!tag} onClick={()=>setTag('')}>{t('homePickAll')}</button>
     {tagChoices.map(option=><button key={option.value} aria-pressed={tag===option.value} onClick={()=>setTag(tag===option.value?'':option.value)}>{option.label}</button>)}
    </div>}
    {chosen?<div className="home-pick">
     <button className="home-pick-main" key={chosen.id} onClick={()=>onSelect(chosen)}>
      <Cover work={chosen} safe={safe}/>
      <span><small>{t('homePickLead')}</small><strong>{titleFor(chosen,titleMode)}</strong><span>{[chosen.developer,chosen.released?.slice(0,4)].filter(Boolean).join(' · ')}</span><span>{tagNames(chosen.tags).slice(0,3).join('、')}</span></span>
     </button>
     <div className="home-pick-others">{others.map(entry=><button key={entry.work.id} aria-label={t('homePickSwap',{title:titleFor(entry.work,titleMode)})} title={titleFor(entry.work,titleMode)} onClick={()=>setPick(entry.index)}><Cover work={entry.work} safe={safe}/></button>)}</div>
    </div>:pool.data&&<p className="home-pick-none">{t('homePickNone')}</p>}
   </section>

   <section className="home-card home-overview" aria-labelledby="home-overview-title">
    <h2 id="home-overview-title">{t('homeOverview')}</h2>
    <p className="home-total"><strong>{total}</strong>{t('works')}</p>
    {total>0&&<div className="home-bar" aria-hidden="true">{segments.filter(segment=>segment.count).map(segment=><span key={segment.key} className={'seg-'+segment.key} style={{flex:segment.count}}/>)}</div>}
    <ul>{segments.map(segment=><li key={segment.key}><button className="metadata-link" onClick={()=>onCollection(segment.key)}><span className={'dot seg-'+segment.key}/>{t(segment.key)}<span>{segment.count}</span></button></li>)}</ul>
   </section>
  </div>

  <Shelf title={t('homeRecent')} works={recent.data?.items.slice(0,14)||[]} safe={safe} titleMode={titleMode} onSelect={onSelect} onAll={()=>onCollection()}/>
  <Shelf title={t('favorite')} works={favorites.data?.items.slice(0,14)||[]} safe={safe} titleMode={titleMode} onSelect={onSelect} onAll={()=>onCollection('favorite')}/>
 </div>;
}

function Shelf({title,works,safe,titleMode,onSelect,onAll}:{title:string;works:Work[];safe:boolean;titleMode:TitleMode;onSelect:(w:Work)=>void;onAll:()=>void}){
 const {t}=useTranslation();
 if(!works.length)return null;
 return <section className="home-shelf">
  <header><h2>{title}</h2><button className="text-button" onClick={onAll}>{t('homeShowAll')}<More size={15}/></button></header>
  <div className="home-shelf-row">{works.map(work=><button key={work.id} className="home-shelf-item" onClick={()=>onSelect(work)}>
   <Cover work={work} safe={safe}/>
   <strong>{titleFor(work,titleMode)}</strong>
   <span>{work.developer}</span>
  </button>)}</div>
 </section>;
}
