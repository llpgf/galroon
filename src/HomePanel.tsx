import {useEffect,useMemo,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ChevronRight as More,Shuffle,Heart} from 'lucide-react';
import type {Work} from './api';
import {CachedImage} from './CachedImage';
import {useCollectionPage,type CollectionQuery} from './useCollectionPage';
import {emptyFilters,titleFor,coverTone,type Filters} from './library';

type TitleMode='display'|'original';
type StatusView='tabs'|'board';
const STATUSES=['playing','completed','backlog','on_hold','dropped'] as const;
type Status=typeof STATUSES[number];
const HERO_POOL=6,HAND=3,TAB_COVERS=8,BOARD_ROWS=3;
const VIEW_KEY='galroon.homeStatusView';

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

const byline=(work:Work)=>[work.developer,work.released?.slice(0,4)].filter(Boolean).join(' · ');
const randomOther=(current:number,size:number)=>{if(size<2)return current;let next=current;while(next===current)next=Math.floor(Math.random()*size);return next;};
function randomHand(size:number){const picks:number[]=[];while(picks.length<Math.min(HAND,size)){const n=Math.floor(Math.random()*size);if(!picks.includes(n))picks.push(n);}return picks;}
function savedView():StatusView{try{return localStorage.getItem(VIEW_KEY)==='board'?'board':'tabs';}catch{return 'tabs';}}

/**
 * Home: a bento of the organised collection. A random recent addition, a hand of backlog cards to pick
 * the next game from, play status as tabs or a board, recent additions, favourites and top brands.
 * Everything is read from the same paged collection API the collection page uses; nothing here changes the collection.
 */
export function HomePanel({generation,safe,titleMode,onSelect,onCollection}:{generation:number;safe:boolean;titleMode:TitleMode;onSelect:(w:Work)=>void;onCollection:(status?:string)=>void}){
 const {t}=useTranslation();
 const recent=useWorks(emptyFilters,'all','added',titleMode,generation);
 const favorites=useWorks({...emptyFilters,favorites:true},'all','added',titleMode,generation);
 const counts=recent.data?.status_counts||{},total=recent.data?.total??0;

 const heroes=recent.data?.items.slice(0,HERO_POOL)||[];
 const [hero,setHero]=useState(()=>Math.floor(Math.random()*HERO_POOL));
 const featured=heroes.length?heroes[hero%heroes.length]:null;

 const [tag,setTag]=useState('');
 const pickFilters=useMemo<Filters>(()=>({...emptyFilters,availability:'available',tags:tag?[tag]:[]}),[tag]);
 const pool=useWorks(pickFilters,'backlog','added',titleMode,generation);
 const tagChoices=useMemo(()=>{const tagCounts=recent.data?.tag_counts||{};return (recent.data?.options.tag||[]).filter(option=>tagCounts[option.value]).sort((a,b)=>(tagCounts[b.value]||0)-(tagCounts[a.value]||0)).slice(0,5);},[recent.data]);
 const items=pool.data?.items||[];
 const [hand,setHand]=useState<number[]>([]),[front,setFront]=useState(0),[deal,setDeal]=useState(0);
 useEffect(()=>{setHand(randomHand(items.length));setFront(0);},[pool.data]);
 const cards=hand.filter(index=>index<items.length).map(index=>items[index]);
 const chosen=cards[front]||null;
 const shuffle=()=>{if(items.length<2)return;setHand(randomHand(items.length));setFront(0);setDeal(n=>n+1);};

 const [view,setView]=useState<StatusView>(savedView);
 const changeView=(next:StatusView)=>{setView(next);try{localStorage.setItem(VIEW_KEY,next);}catch{/* the choice just isn't remembered */}};
 const [status,setStatus]=useState<Status>('playing');

 const studios=useMemo(()=>{const studioCounts=recent.data?.studio_counts||{};return (recent.data?.options.studio||[]).map(option=>({label:option.label,count:studioCounts[option.value]||0})).filter(entry=>entry.count).sort((a,b)=>b.count-a.count).slice(0,4);},[recent.data]);

 return <div className="home">
  <div className="home-top"><h1>{t('homeGreeting')}</h1></div>

  <div className="home-bento">
   {featured&&<section className="home-tile home-hero" aria-labelledby="home-hero-title">
    <div className="home-hero-backdrop" aria-hidden="true">{featured.cover&&!safe&&<CachedImage key={featured.id} source={featured.cover}/>}</div>
    <button className="home-hero-art" key={'art-'+featured.id} tabIndex={-1} aria-hidden="true" onClick={()=>onSelect(featured)}><Cover work={featured} safe={safe}/></button>
    <div className="home-hero-copy" key={featured.id}>
     <h2 id="home-hero-title" className="home-eyebrow">{t('homeRandomRecent')}</h2>
     <strong>{titleFor(featured,titleMode)}</strong>
     <span>{byline(featured)}</span>
    </div>
    <div className="home-hero-actions">
     <button className="home-hero-primary" onClick={()=>onSelect(featured)}>{t('homeViewWork')}</button>
     {heroes.length>1&&<button className="home-hero-secondary" onClick={()=>setHero(randomOther(hero%heroes.length,heroes.length))}>{t('homeRandomAnother')}</button>}
     {heroes.length>1&&<span className="home-dots" aria-hidden="true">{heroes.map((work,index)=><span key={work.id} className={index===hero%heroes.length?'on':undefined}/>)}</span>}
    </div>
   </section>}

   <section className="home-tile home-picker" aria-labelledby="home-pick-title">
    <div className="home-hand" aria-label={t('homePickHand')}>
     {cards.map((work,index)=>{const position=(index-front+cards.length)%cards.length;return <button key={deal+':'+work.id} className={'home-card-'+position} style={{zIndex:cards.length-position,animationDelay:position*70+'ms'}} aria-label={position===0?t('openWorkNamed',{name:titleFor(work,titleMode)}):t('homePickSwap',{title:titleFor(work,titleMode)})} onClick={()=>position===0?onSelect(work):setFront(index)}><Cover work={work} safe={safe}/></button>;})}
    </div>
    <div className="home-pick-copy">
     <h2 id="home-pick-title" className="home-eyebrow">{t('homePickTitle')} · {pool.data?t('homePickCount',{count:pool.data.filtered}):t('loading')}</h2>
     {chosen?<><strong>{titleFor(chosen,titleMode)}</strong><span>{byline(chosen)}</span>{cards.length>1&&<small>{t('homePickBackHint')}</small>}</>:pool.data&&<p className="home-pick-none">{t('homePickNone')}</p>}
     {tagChoices.length>0&&<div className="home-chips">
      <button aria-pressed={!tag} onClick={()=>setTag('')}>{t('homePickAll')}</button>
      {tagChoices.map(option=><button key={option.value} aria-pressed={tag===option.value} title={option.label} onClick={()=>setTag(tag===option.value?'':option.value)}><span>{option.label}</span></button>)}
     </div>}
    </div>
    <button className="home-shuffle primary" aria-label={t('homePickDeal')} title={t('homePickDeal')} disabled={items.length<2} onClick={shuffle}><Shuffle size={22}/></button>
   </section>

   <section className="home-tile home-status" aria-labelledby="home-status-title">
    <header>
     <div><h2 id="home-status-title">{t('homeMyPlay')}</h2><span>{t('homeTotal',{count:total})}</span></div>
     <div className="home-status-tools">
      <div className="tabs" role="radiogroup" aria-label={t('homeViewMode')}>{(['tabs','board'] as const).map(value=><button key={value} role="radio" aria-checked={view===value} aria-pressed={view===value} onClick={()=>changeView(value)}>{t(value==='tabs'?'homeViewTabs':'homeViewBoard')}</button>)}</div>
      <button className="text-button" onClick={()=>onCollection()}>{t('homeViewInCollection')}<More size={15}/></button>
     </div>
    </header>
    {total>0&&<div className="home-bar" aria-hidden="true">{STATUSES.filter(key=>counts[key]).map(key=><span key={key} className={'seg-'+key} style={{flex:counts[key],opacity:view==='tabs'&&key!==status?.45:1}}/>)}</div>}
    {view==='tabs'?<StatusTabs status={status} setStatus={setStatus} counts={counts} generation={generation} safe={safe} titleMode={titleMode} onSelect={onSelect} onCollection={onCollection}/>
     :<div className="home-board">{STATUSES.map(key=><StatusColumn key={key} status={key} count={counts[key]||0} generation={generation} safe={safe} titleMode={titleMode} onSelect={onSelect} onCollection={onCollection}/>)}</div>}
   </section>
  </div>

  <Shelf title={t('homeRecent')} works={recent.data?.items.slice(0,14)||[]} safe={safe} titleMode={titleMode} onSelect={onSelect} onAll={()=>onCollection()}/>

  <div className="home-bento">
   {!!favorites.data?.items.length&&<section className="home-tile home-favorites" aria-labelledby="home-fav-title">
    <header><h2 id="home-fav-title">{t('favorite')}</h2><button className="text-button" onClick={()=>onCollection('favorite')}>{t('homeShowAll')}<More size={15}/></button></header>
    <div>{favorites.data.items.slice(0,6).map(work=><button key={work.id} className="home-mini" onClick={()=>onSelect(work)}><Cover work={work} safe={safe}/><span>{titleFor(work,titleMode)}</span></button>)}</div>
   </section>}
   {studios.length>0&&<section className="home-tile home-studios" aria-labelledby="home-studio-title">
    <h2 id="home-studio-title">{t('homeTopStudios')}</h2>
    <ul>{studios.map(entry=><li key={entry.label}><span>{entry.label}</span><span className="home-meter" aria-hidden="true"><span style={{width:entry.count/studios[0].count*100+'%'}}/></span><span>{entry.count}</span></li>)}</ul>
   </section>}
  </div>
 </div>;
}

type StatusProps={generation:number;safe:boolean;titleMode:TitleMode;onSelect:(w:Work)=>void;onCollection:(status?:string)=>void};

function StatusTabs({status,setStatus,counts,generation,safe,titleMode,onSelect,onCollection}:StatusProps&{status:Status;setStatus:(s:Status)=>void;counts:Record<string,number>}){
 const {t}=useTranslation();
 const page=useWorks(emptyFilters,status,'added',titleMode,generation);
 const works=page.data?.items.slice(0,TAB_COVERS)||[];
 return <>
  <div className="home-status-tabs" role="tablist" aria-label={t('focusStatus')}>{STATUSES.map(key=><button key={key} role="tab" id={'home-tab-'+key} aria-selected={status===key} aria-controls="home-tab-panel" className={'home-status-tab tab-'+key} onClick={()=>setStatus(key)}><span><span className={'dot seg-'+key}/>{t(key)}</span><strong>{counts[key]||0}</strong></button>)}</div>
  <div className="home-tab-panel" id="home-tab-panel" role="tabpanel" aria-labelledby={'home-tab-'+status}>
   {works.length?<>
    <div className="home-tab-covers">{works.map(work=><button key={work.id} className={'home-mini'+(status==='dropped'?' dimmed':'')} onClick={()=>onSelect(work)}><Cover work={work} safe={safe}/><span>{titleFor(work,titleMode)}</span></button>)}</div>
    <button className="text-button" onClick={()=>onCollection(status)}>{t('homeStatusAll',{count:counts[status]||0})}<More size={15}/></button>
   </>:page.data&&<p className="home-status-empty">{t('homeStatusEmpty')}</p>}
  </div>
 </>;
}

function StatusColumn({status,count,generation,safe,titleMode,onSelect,onCollection}:StatusProps&{status:Status;count:number}){
 const {t}=useTranslation();
 const page=useWorks(emptyFilters,status,'added',titleMode,generation);
 const works=page.data?.items.slice(0,BOARD_ROWS)||[];
 return <section className={'home-column column-'+status} aria-label={t(status)}>
  <button className="home-column-head" onClick={()=>onCollection(status)}><span><span className={'dot seg-'+status}/>{t(status)}</span><strong>{count}</strong></button>
  <span className={'home-column-rule seg-'+status} aria-hidden="true"/>
  {works.map(work=><button key={work.id} className="home-row" onClick={()=>onSelect(work)}><Cover work={work} safe={safe}/><span><strong>{titleFor(work,titleMode)}</strong><span>{work.developer}</span></span></button>)}
  {page.data&&!works.length&&<p className="home-status-empty">{t('homeStatusEmpty')}</p>}
  {count>works.length&&<button className="text-button home-column-more" onClick={()=>onCollection(status)}>{t('homeStatusMore',{count:count-works.length})}<More size={15}/></button>}
 </section>;
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
