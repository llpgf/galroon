import {useState,type DragEvent,type KeyboardEvent,type PointerEvent} from 'react';
import {useTranslation} from 'react-i18next';
import {ChevronRight as More,GripVertical,Heart} from 'lucide-react';
import type {Work} from './api';
import {CachedImage} from './CachedImage';
import {ReleaseCalendar} from './ReleaseCalendar';
import {useCollectionPage,type CollectionQuery} from './useCollectionPage';
import {emptyFilters,titleFor,coverTone,type Filters} from './library';

type TitleMode='display'|'original';
type StatusView='tabs'|'board';
const STATUSES=['playing','completed','backlog','on_hold','dropped'] as const;
type Status=typeof STATUSES[number];
const HERO_POOL=6,TAB_COVERS=8,BOARD_ROWS=3,FAVORITES=8;
const VIEW_KEY='galroon.homeStatusView',ORDER_KEY='galroon.homeStatusOrder';

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

/** Glass lens: the pointer position drives a sharp, magnified circle over the blurred hero backdrop (CSS reads --mx/--my). */
const followLens=(event:PointerEvent<HTMLElement>)=>{if(event.pointerType!=='mouse')return;const box=event.currentTarget.getBoundingClientRect();event.currentTarget.style.setProperty('--mx',`${event.clientX-box.left}px`);event.currentTarget.style.setProperty('--my',`${event.clientY-box.top}px`);};
const byline=(work:Work)=>[work.developer,work.released?.slice(0,4)].filter(Boolean).join(' · ');
/** A random batch of up to HERO_POOL ids, taking ids outside the current batch first so a new batch looks new. */
function randomBatch(ids:string[],current:string[]){
 const shuffle=(list:string[])=>{const out=[...list];for(let i=out.length-1;i>0;i--){const j=Math.floor(Math.random()*(i+1));[out[i],out[j]]=[out[j],out[i]];}return out;};
 return [...shuffle(ids.filter(id=>!current.includes(id))),...shuffle(ids.filter(id=>current.includes(id)))].slice(0,HERO_POOL);
}
function savedView():StatusView{try{return localStorage.getItem(VIEW_KEY)==='board'?'board':'tabs';}catch{return 'tabs';}}
/** A saved order is used only when it is exactly the five statuses, so a stale or edited value can't drop one. */
function savedOrder():Status[]{try{const saved=JSON.parse(localStorage.getItem(ORDER_KEY)||'null');if(Array.isArray(saved)&&saved.length===STATUSES.length&&STATUSES.every(s=>saved.includes(s)))return saved;}catch{/* fall back to the default order */}return [...STATUSES];}

/**
 * Home: a bento of the organised collection. A random recent addition, the release calendar, play status
 * as tabs or a board whose columns can be reordered, recent additions and favourites.
 * Collection data comes from the same paged collection API the collection page uses; nothing here changes the collection.
 */
export function HomePanel({generation,safe,titleMode,onSelect,onCollection}:{generation:number;safe:boolean;titleMode:TitleMode;onSelect:(w:Work)=>void;onCollection:(status?:string)=>void}){
 const {t}=useTranslation();
 const recent=useWorks(emptyFilters,'all','added',titleMode,generation);
 const favorites=useWorks({...emptyFilters,favorites:true},'all','added',titleMode,generation);
 const counts=recent.data?.status_counts||{},total=recent.data?.total??0;

 // The hero rotates through a batch of six: the newest additions first, then a random six from the loaded recent page.
 const pool=recent.data?.items||[];
 const [batch,setBatch]=useState<string[]|null>(null);
 const picked=batch&&batch.map(id=>pool.find(work=>work.id===id)).filter((work):work is Work=>!!work);
 const heroes=picked?.length?picked:pool.slice(0,HERO_POOL);
 const [hero,setHero]=useState(()=>Math.floor(Math.random()*HERO_POOL));
 const current=hero%Math.max(heroes.length,1),featured=heroes.length?heroes[current]:null;
 const nextBatch=()=>{setBatch(randomBatch(pool.map(work=>work.id),heroes.map(work=>work.id)));setHero(0);};

 const [view,setView]=useState<StatusView>(savedView);
 const changeView=(next:StatusView)=>{setView(next);try{localStorage.setItem(VIEW_KEY,next);}catch{/* the choice just isn't remembered */}};
 const [status,setStatus]=useState<Status>('playing');
 const [order,setOrder]=useState<Status[]>(savedOrder);
 const [dragging,setDragging]=useState<Status|null>(null),[over,setOver]=useState<Status|null>(null);
 const swap=(a:Status,b:Status)=>{setDragging(null);setOver(null);if(a===b)return;const next=order.map(s=>s===a?b:s===b?a:s);setOrder(next);try{localStorage.setItem(ORDER_KEY,JSON.stringify(next));}catch{/* the order just isn't remembered */}};
 const drag={dragging,over,setDragging,setOver,swap,order};

 return <div className="home">
  <div className="home-top"><h1>{t('homeGreeting')}</h1></div>

  <div className="home-bento">
   {featured&&<section className="home-tile home-hero" aria-labelledby="home-hero-title" onPointerMove={followLens}>
    <div className="home-hero-backdrop" aria-hidden="true">{featured.cover&&!safe&&<CachedImage key={featured.id} source={featured.cover}/>}</div>
    {featured.cover&&!safe&&<div className="home-hero-lens" aria-hidden="true"><div><CachedImage key={featured.id} source={featured.cover}/></div></div>}
    <div className="home-hero-glow" aria-hidden="true"/>
    <button className="home-hero-art" key={'art-'+featured.id} tabIndex={-1} aria-hidden="true" onClick={()=>onSelect(featured)}><Cover work={featured} safe={safe}/></button>
    <div className="home-hero-copy" key={featured.id}>
     <h2 id="home-hero-title" className="home-eyebrow">{t('homeRandomRecent')}</h2>
     <strong>{titleFor(featured,titleMode)}</strong>
     <span>{byline(featured)}</span>
    </div>
    <div className="home-hero-actions">
     <button className="home-hero-primary" onClick={()=>onSelect(featured)}>{t('homeViewWork')}</button>
     {pool.length>HERO_POOL&&<button className="home-hero-secondary" onClick={nextBatch}>{t('homeRandomAnother')}</button>}
     {heroes.length>1&&<span className="home-dots" role="group" aria-label={t('homeHeroPicker')}>{heroes.map((work,index)=><button key={work.id} aria-label={titleFor(work,titleMode)} aria-current={index===current||undefined} onClick={()=>setHero(index)}><span className={index===current?'on':undefined}/></button>)}</span>}
    </div>
   </section>}

   <ReleaseCalendar safe={safe} titleMode={titleMode} onSelect={onSelect}/>

   <section className="home-tile home-status" aria-labelledby="home-status-title">
    <header>
     <div><h2 id="home-status-title">{t('homeMyPlay')}</h2><span>{t('homeTotal',{count:total})}</span><button className="text-button" onClick={()=>onCollection('none')}>{t('homeNoStatus',{count:counts.none||0})}</button></div>
     <div className="home-status-tools">
      <div className="tabs" role="radiogroup" aria-label={t('homeViewMode')}>{(['tabs','board'] as const).map(value=><button key={value} role="radio" aria-checked={view===value} aria-pressed={view===value} onClick={()=>changeView(value)}>{t(value==='tabs'?'homeViewTabs':'homeViewBoard')}</button>)}</div>
      <button className="text-button" onClick={()=>onCollection()}>{t('homeViewInCollection')}<More size={15}/></button>
     </div>
    </header>
    {total>0&&<div className="home-bar" aria-hidden="true">{order.filter(key=>counts[key]).map(key=><span key={key} className={'seg-'+key} style={{flex:counts[key],opacity:view==='tabs'&&key!==status?.45:1}}/>)}</div>}
    {view==='tabs'?<StatusTabs order={order} status={status} setStatus={setStatus} counts={counts} generation={generation} safe={safe} titleMode={titleMode} onSelect={onSelect} onCollection={onCollection}/>
     :<>
      <p className="home-board-hint">{t('homeBoardHint')}</p>
      <div className="home-board">{order.map((key,index)=><StatusColumn key={key} status={key} index={index} count={counts[key]||0} drag={drag} generation={generation} safe={safe} titleMode={titleMode} onSelect={onSelect} onCollection={onCollection}/>)}</div>
     </>}
   </section>
  </div>

  <Shelf title={t('homeRecent')} works={recent.data?.items.slice(0,14)||[]} safe={safe} titleMode={titleMode} onSelect={onSelect} onAll={()=>onCollection()}/>

  {!!favorites.data?.items.length&&<section className="home-tile home-favorites" aria-labelledby="home-fav-title">
   <header><h2 id="home-fav-title">{t('favorite')}</h2><button className="text-button" onClick={()=>onCollection('favorite')}>{t('homeShowAll')}<More size={15}/></button></header>
   <div>{favorites.data.items.slice(0,FAVORITES).map(work=><button key={work.id} className="home-mini" onClick={()=>onSelect(work)}><Cover work={work} safe={safe}/><span>{titleFor(work,titleMode)}</span></button>)}</div>
  </section>}
 </div>;
}

type StatusProps={generation:number;safe:boolean;titleMode:TitleMode;onSelect:(w:Work)=>void;onCollection:(status?:string)=>void};
type Drag={dragging:Status|null;over:Status|null;setDragging:(s:Status|null)=>void;setOver:(s:Status|null)=>void;swap:(a:Status,b:Status)=>void;order:Status[]};

function StatusTabs({order,status,setStatus,counts,generation,safe,titleMode,onSelect,onCollection}:StatusProps&{order:Status[];status:Status;setStatus:(s:Status)=>void;counts:Record<string,number>}){
 const {t}=useTranslation();
 const page=useWorks(emptyFilters,status,'added',titleMode,generation);
 const works=page.data?.items.slice(0,TAB_COVERS)||[];
 return <>
  <div className="home-status-tabs" role="tablist" aria-label={t('focusStatus')}>{order.map(key=><button key={key} role="tab" id={'home-tab-'+key} aria-selected={status===key} aria-controls="home-tab-panel" className={'home-status-tab tab-'+key} onClick={()=>setStatus(key)}><span><span className={'dot seg-'+key}/>{t(key)}</span><strong>{counts[key]||0}</strong></button>)}</div>
  <div className="home-tab-panel" id="home-tab-panel" role="tabpanel" aria-labelledby={'home-tab-'+status}>
   {works.length?<>
    <div className="home-tab-covers">{works.map(work=><button key={work.id} className={'home-mini'+(status==='dropped'?' dimmed':'')} onClick={()=>onSelect(work)}><Cover work={work} safe={safe}/><span>{titleFor(work,titleMode)}</span></button>)}</div>
    <button className="text-button" onClick={()=>onCollection(status)}>{t('homeStatusAll',{count:counts[status]||0})}<More size={15}/></button>
   </>:page.data&&<p className="home-status-empty">{t('homeStatusEmpty')}</p>}
  </div>
 </>;
}

/** A board column. The whole column is a drag source and drop target; its grip moves it with ← and →. */
function StatusColumn({status,index,count,drag,generation,safe,titleMode,onSelect,onCollection}:StatusProps&{status:Status;index:number;count:number;drag:Drag}){
 const {t}=useTranslation();
 const page=useWorks(emptyFilters,status,'added',titleMode,generation);
 const works=page.data?.items.slice(0,BOARD_ROWS)||[];
 const target=drag.over===status&&drag.dragging!==null&&drag.dragging!==status;
 const start=(e:DragEvent)=>{e.dataTransfer.effectAllowed='move';e.dataTransfer.setData('text/plain',status);drag.setDragging(status);};
 const enter=(e:DragEvent)=>{if(!drag.dragging)return;e.preventDefault();e.dataTransfer.dropEffect='move';if(drag.over!==status)drag.setOver(status);};
 const drop=(e:DragEvent)=>{e.preventDefault();if(drag.dragging)drag.swap(drag.dragging,status);};
 const key=(e:KeyboardEvent)=>{const step=e.key==='ArrowLeft'?-1:e.key==='ArrowRight'?1:0;const neighbour=drag.order[index+step];if(!step||!neighbour)return;e.preventDefault();drag.swap(status,neighbour);};
 return <section className={'home-column column-'+status+(drag.dragging===status?' dragging':'')+(target?' drop-target':'')} aria-label={t(status)} draggable onDragStart={start} onDragOver={enter} onDragLeave={()=>{if(drag.over===status)drag.setOver(null);}} onDrop={drop} onDragEnd={()=>{drag.setDragging(null);drag.setOver(null);}}>
  <div className="home-column-top">
   <button className="home-column-grip" aria-label={t('homeColumnMove',{status:t(status),position:index+1})} title={t('homeColumnMoveHint')} onKeyDown={key}><GripVertical size={16}/></button>
   <button className="home-column-head" onClick={()=>onCollection(status)}><span><span className={'dot seg-'+status}/>{t(status)}</span><strong>{count}</strong></button>
  </div>
  <span className={'home-column-rule seg-'+status} aria-hidden="true"/>
  {works.map(work=><button key={work.id} className="home-row" draggable={false} onClick={()=>onSelect(work)}><Cover work={work} safe={safe}/><span><strong>{titleFor(work,titleMode)}</strong><span>{work.developer}</span></span></button>)}
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
