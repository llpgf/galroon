import {MatchWorkChoices} from './MatchWorkChoices';
import {OrganizePanel} from './OrganizePanel';
import {CatalogGenerationContext} from './useOwnedWorks';
import {TaskMessages} from './TaskMessages';
import {JobSummary} from './JobSummary';
import {JobHistory,type JobPage} from './JobHistory';
import {APP_VERSION} from './version';
import {ArtworkCachePanel} from './ArtworkCachePanel';
import {CachedImage} from './CachedImage';
import {ListsWorkspace} from './ListsWorkspace';
import {Timeline} from './Timeline';
import {DiagnosticsPanel} from './DiagnosticsPanel';
import {reportClientError} from './clientDiagnostics';
import {IssuesPanel} from './IssuesPanel';
import {MatchHistory} from './MatchHistory';
import {ConnectionsPanel} from './ConnectionsPanel';
import type {DiscoveryQuery} from './discovery';
import {SettingsLayout} from './SettingsLayout';
import {CollectionSearch} from './CollectionSearch';
import {rememberSearch} from './search';
import {collectionStorageKey} from './api';
import {CollectionWorkDetail} from './CollectionWorkDetail';
import {AutoMatchPanel} from './AutoMatchPanel';
import {CollectionContextPanel} from './CollectionContextPanel';
import {ResourceGrouping} from './ResourceGrouping';
import {ConnectionBanner} from './ConnectionBanner';
import {PlanReview} from './PlanReview';
import {PlanHistory} from './PlanHistory';
import {LibraryBrowser} from './LibraryBrowser';
import {HomePanel} from './HomePanel';
import {titleFor} from './library';
import {SourceWatch} from './SourceWatch';
import {AccessPanel} from './AccessPanel';
import {WebLogin} from './WebLogin';
import {DuplicatePanel} from './DuplicatePanel';
import {AcquirePanel} from './AcquirePanel';
import {ResourceReferences} from './ResourceReferences';
import {SourceRemap} from './SourceRemap';
import {BackupPanel} from './BackupPanel';
import {GroupingEditor} from './GroupingEditor';
import {MetadataEditor} from './MetadataEditor';
import React,{useEffect,useState,useCallback,useRef,useSyncExternalStore} from 'react';
import {createRoot} from 'react-dom/client';
import {installDrawerKeys} from './drawerKeys';
import {resolveLanguage} from './i18n';import './theme.css';
import {DisplayContext,type NameMode} from './display';
import {applyTheme,storedTheme,type ThemeChoice} from './themeChoice';
import {useTranslation} from 'react-i18next';
import {House,Sun,Moon,Monitor,SunMoon,Languages,Inbox,BookOpen,LibraryBig,Folder,FolderOpen,Layers,Activity,Archive,Settings,Search,Plus,ArrowUpRight,ChevronRight,ChevronLeft,X,Heart,Pause,Play,Square,Check,ShieldCheck,HardDrive,RefreshCw,Globe,EyeOff} from 'lucide-react';
import {requireCollectionPaging,ConnectionLostError,connectionSnapshot,subscribeConnection,AuthenticationError,signOut,type AccessIdentity,type CollectionContext,SessionChangedError,sessionChanging,isDesktop,stopCore,api,connect,folder,bytes,type Work,type Resource,type Root,type Job,type Plan,type Isolated} from './api';
const ACTIVE_JOB_STATES=new Set(['queued','running','pausing','cancelling','executing']);
type View='home'|'lists'|'library'|'organize'|'sources'|'tasks'|'quarantine'|'settings';
type Candidate={id:string;title:string;alttitle?:string;description?:string;image?:{url:string};released?:string;developers?:{name:string}[];tags?:{name:string}[];match?:{reason:string;matched_title:string;strength:string;ambiguous?:boolean}};
function App(){
 const [initialSearch,setInitialSearch]=useState<DiscoveryQuery|null>(null);
 const {t,i18n}=useTranslation();const [ready,setReady]=useState(false),[view,setView]=useState<View>('home'),[error,setError]=useState(''),[busy,setBusy]=useState(false),[query,setQuery]=useState('');
 const coreConnection=useSyncExternalStore(subscribeConnection,connectionSnapshot);
 const reportError=(e:unknown)=>{reportClientError(e);if(isDesktop&&e instanceof ConnectionLostError)return;if(e instanceof AuthenticationError&&!isDesktop)setReady(false);if(!(e instanceof SessionChangedError))setError(e instanceof Error?e.message:String(e));};
 const [identity,setIdentity]=useState<AccessIdentity|null>(null);const canWrite=identity?.can_write===true;const [context,setContext]=useState<CollectionContext|null>(null);
 const [roots,setRoots]=useState<Root[]>([]),[jobPage,setJobPage]=useState<JobPage>({items:[],next:null}),[plans,setPlans]=useState<Plan[]>([]),[isolated,setIsolated]=useState<Isolated[]>([]);
 const jobs=jobPage.items;
 const [collectionGeneration,setCollectionGeneration]=useState(0),[collectionSummary,setCollectionSummary]=useState({works:0,unassigned:0});const collectionSignature=useRef('');
 const [titleMode,setTitleModeValue]=useState<'display'|'original'>(localStorage.getItem('galroon.titleMode')==='original'?'original':'display');
 const setTitleMode=(mode:'display'|'original')=>{setTitleModeValue(mode);localStorage.setItem('galroon.titleMode',mode);};
 const [nameMode,setNameModeValue]=useState<NameMode>(localStorage.getItem('galroon.nameMode')==='original'?'original':'romanized');
 const setNameMode=(mode:NameMode)=>{setNameModeValue(mode);localStorage.setItem('galroon.nameMode',mode);};
 const [theme,setThemeValue]=useState<ThemeChoice>(storedTheme());
 const setTheme=(choice:ThemeChoice)=>{setThemeValue(choice);applyTheme(choice);};
 const [filter,setFilter]=useState('all'),[safe,setSafe]=useState(localStorage.getItem('galroon.safe')===null?(!isDesktop&&window.matchMedia('(max-width:700px)').matches):localStorage.getItem('galroon.safe')==='true'),[detail,setDetail]=useState<Work|null>(null),[matcher,setMatcher]=useState<Resource|null>(null),[manual,setManual]=useState(false),[candidates,setCandidates]=useState<Candidate[]>([]),[matchQuery,setMatchQuery]=useState(''),[matchInfo,setMatchInfo]=useState<{searched:string[];incomplete:boolean;warning?:string;needs_title?:boolean}|null>(null),[edition,setEdition]=useState('Unclassified edition');
 const [newSource,setNewSource]=useState(false),[path,setPath]=useState(''),[label,setLabel]=useState(''),[scanRoot,setScanRoot]=useState<Root|null>(null),[scope,setScope]=useState(''),[excludes,setExcludes]=useState(''),[plan,setPlan]=useState<Plan|null>(null);

 const [acquiring,setAcquiring]=useState<Resource|null>(null),[password,setPassword]=useState('');
 const matchEpoch=useRef(0);const jobSignature=useRef('');const polling=useRef(false);
 const resumeChanged=useRef<()=>Promise<void>>(async()=>{});
 const [resuming,setResuming]=useState<Job|null>(null);
 const [groupingWork,setGroupingWork]=useState<Work|null>(null);
 const [historyResource,setHistoryResource]=useState<Pick<Resource,'id'|'title'|'path'>|null>(null);
 const [regroupResource,setRegroupResource]=useState<Resource|null>(null);
 const [referenceResource,setReferenceResource]=useState<Resource|null>(null);
 const [remapping,setRemapping]=useState<Root|null>(null);
 const [editingMetadata,setEditingMetadata]=useState<Work|null>(null);
 const detailTarget=useRef<string|null>(null);detailTarget.current=detail?.id||null;
 const reloadWork=async(id:string)=>{const updated=await api<Work>(`/works/${encodeURIComponent(id)}`);setDetail(old=>old?.id===id&&old.revision<=updated.revision?updated:old);setCollectionGeneration(n=>n+1);};
 const refresh=useCallback(async()=>{const target=detailTarget.current;const [s,j,p,q,access,context,summary,currentWork]=await Promise.all([api<Root[]>('/roots'),api<JobPage>('/jobs?paged=true'),api<Plan[]>('/plans'),api<Isolated[]>('/quarantine'),api<AccessIdentity>('/access/me'),api<CollectionContext>('/context'),api<{works:number;unassigned:number;revision:number}>('/dashboard'),target?api<Work>(`/works/${encodeURIComponent(target)}`).catch(e=>{if(e instanceof Error&&e.message==='Work not found or merged')return null;throw e;}):Promise.resolve(undefined)]);requireCollectionPaging(context);setContext(context);setIdentity(access);if(currentWork!==undefined){setDetail(old=>old?.id===target?currentWork&&old.revision>currentWork.revision?old:currentWork:old);}setRoots(s);setJobPage(j);setPlans(p);setIsolated(q);setCollectionSummary(summary);const signature=JSON.stringify([context.library.id,summary.revision,s.map(root=>[root.id,root.online])]);if(signature!==collectionSignature.current){collectionSignature.current=signature;setCollectionGeneration(n=>n+1);}},[]);
 useEffect(()=>{if(ready&&detail?.id)void refresh().catch(reportError);},[detail?.id,ready,refresh]);
 const act=async(fn:()=>Promise<unknown>)=>{setBusy(true);setError('');try{await fn();await refresh();}catch(e){reportError(e);}finally{setBusy(false);}};
 useEffect(()=>{connect().then(ok=>{if(ok&&!isDesktop){refresh().then(()=>setReady(true)).catch(reportError);}}).catch(reportError);},[refresh]);
 useEffect(()=>{if(isDesktop&&coreConnection.phase==='connected'){void refresh().then(()=>setReady(true)).catch(reportError);}},[coreConnection,refresh]);
 useEffect(()=>{if(ready&&canWrite)void api('/matching/start',{}).catch(reportError);},[ready,canWrite]);
 // Poll quickly while a task is active; otherwise only often enough to notice tasks started elsewhere.
 useEffect(()=>{if(!ready)return;let live=true,active=true,timer:number|undefined;const schedule=()=>{if(live)timer=window.setTimeout(tick,document.hidden?15000:active?1300:4000);};const tick=()=>{if(sessionChanging()||polling.current){schedule();return;}polling.current=true;api<JobPage>('/jobs?paged=true').then(j=>{setJobPage(j);active=j.items.some(x=>ACTIVE_JOB_STATES.has(x.state));const signature=j.items.map(x=>x.id+':'+x.state+(x.kind==='match'?':'+x.processed:'')).join('|');if(signature!==jobSignature.current){jobSignature.current=signature;void refresh().catch(reportError);}}).catch(reportError).finally(()=>{polling.current=false;schedule();});};const wake=()=>{if(!document.hidden){window.clearTimeout(timer);schedule();}};schedule();document.addEventListener('visibilitychange',wake);return()=>{live=false;window.clearTimeout(timer);document.removeEventListener('visibilitychange',wake);};},[ready,refresh]);
 const collectionY=useRef(0);
 const [scrolled,setScrolled]=useState(false);
 // "More" menus are <details>; close them on outside clicks and Escape like a normal menu.
 useEffect(()=>{const close=(keep?:EventTarget|null)=>document.querySelectorAll<HTMLDetailsElement>('details.toolbar-menu[open]').forEach(menu=>{if(!(keep instanceof Node&&menu.contains(keep)))menu.open=false;});const onClick=(e:MouseEvent)=>close(e.target);const onKey=(e:KeyboardEvent)=>{if(e.key==='Escape')close();};document.addEventListener('click',onClick);document.addEventListener('keydown',onKey);return()=>{document.removeEventListener('click',onClick);document.removeEventListener('keydown',onKey);};},[]);
 useEffect(()=>{const onScroll=()=>setScrolled(window.scrollY>4);onScroll();window.addEventListener('scroll',onScroll,{passive:true});return()=>window.removeEventListener('scroll',onScroll);},[]);
 const selectWork=(w:Work)=>{setInitialSearch(null);rememberSearch(query,collectionStorageKey('search-history')); if(!detail)collectionY.current=window.scrollY;setDetail(w);requestAnimationFrame(()=>window.scrollTo(0,0));};
 const backCollection=()=>{setDetail(null);requestAnimationFrame(()=>window.scrollTo(0,collectionY.current));};
 const navigate=(v:View)=>{setView(v);setDetail(null);setFilter('all');setQuery('');requestAnimationFrame(()=>window.scrollTo(0,0));};

 const startMatch=(r:Resource)=>{matchEpoch.current++;setMatcher(r);setManual(false);setMatchQuery(r.title);setEdition(r.release);setCandidates([]);setMatchInfo(null);};
 const chooseWork=async(wid:string)=>{if(matcher)await api(`/resources/${matcher.id}/bind`,{work_id:wid,release:edition,revision:matcher.revision});setMatcher(null);setManual(false);};
 const createCandidate=async(c?:Candidate)=>{const value=c?{title:c.title,original_title:c.alttitle||c.title,vndb_id:c.id,description:c.description||'',cover:c.image?.url||'',developer:c.developers?.map(d=>d.name).join(', ')||'',released:c.released||'',tags:c.tags?.map(x=>x.name)||[]}:{title:matchQuery,original_title:matchQuery};const wid=(await api<{id:string}>('/works',value)).id;await chooseWork(wid);};
 const pickPlan=async(r:Resource)=>{const dest=await folder();if(dest){const p=await api<Plan>('/plans/organize',{resource_id:r.id,destination:dest});setPlan(p);}};
 const renderLocalWork=(work:Work,onBack:()=>void,onWork:(w:Work)=>void,initialSearch:DiscoveryQuery|null=null,backLabel='back')=><CollectionWorkDetail key={work.id+(initialSearch?.value||'')} work={work} safe={safe} canWrite={canWrite} titleMode={titleMode} initialSearch={initialSearch} backLabel={backLabel} onBack={onBack} onWork={onWork} roots={roots} busy={busy} act={act} refresh={refresh} onEdit={setEditingMetadata} onGroup={setGroupingWork} onReferences={setReferenceResource} onAcquire={r=>{setAcquiring(r);setPassword('');}} onOrganize={r=>void act(()=>pickPlan(r))}/>;
 const active=jobs.filter(j=>['running','queued','pausing'].includes(j.state)).length;
 if(!ready)return <div className="connect"><div className="wordmark">galroon<span className="seal" aria-hidden="true"/></div><h1>{t('tagline')}</h1>{isDesktop?<><ConnectionBanner state={coreConnection}/><p>{t('connectionHelp')}</p><button className="primary" disabled={busy||['connecting','reconnecting','restoring','stopping'].includes(coreConnection.phase)} onClick={()=>void connect().catch(reportError)}>{t('reconnectCore')}</button>{error&&<p role="alert">{error}</p>}</>:<>{error&&<p role="alert">{error}</p>}<WebLogin onConnected={async()=>{setError('');await refresh();setReady(true);}}/></>}</div>;
 return <DisplayContext.Provider value={{names:nameMode,titles:titleMode}}><CatalogGenerationContext.Provider value={collectionGeneration}><><a className="skip-link" href="#main-content">{t('skipToContent')}</a><ConnectionBanner state={coreConnection}/><div className="shell" inert={coreConnection.phase==='collection_changed'||isDesktop&&coreConnection.phase!=='connected'}><aside className="sidebar"><div className="wordmark">galroon<span className="seal" aria-hidden="true"/></div><nav aria-label={t('primaryNavigation')}>{([['navGroupLibrary',[['home',House],['library',LibraryBig],['lists',Layers]]],['navGroupWork',[['organize',Inbox],['tasks',Activity]]],['navGroupStorage',[['sources',Folder],['quarantine',Archive]]]] as const).map(([group,items])=><React.Fragment key={group}><div className="nav-group" aria-hidden="true">{t(group)}</div>{items.filter(([v])=>canWrite||!['organize','quarantine'].includes(v)).map(([v,Icon])=><button key={v} aria-current={view===v?'page':undefined} className={view===v?'nav active':'nav'} onClick={()=>navigate(v)}><Icon size={18}/>{t(v)}{v==='organize'&&collectionSummary.unassigned>0&&<small>{collectionSummary.unassigned}</small>}{v==='tasks'&&active>0&&<small className="live">{active}</small>}</button>)}</React.Fragment>)}<button aria-current={view==='settings'?'page':undefined} className={view==='settings'?'nav active mobile-settings':'nav mobile-settings'} onClick={()=>navigate('settings')}><Settings size={18}/>{t('settings')}</button></nav><div className="sidebar-bottom"><button aria-current={view==='settings'?'page':undefined} className={view==='settings'?'nav active':'nav'} onClick={()=>navigate('settings')}><Settings size={18}/>{t('settings')}</button><div className="theme-switch" role="radiogroup" aria-label={t('themeSetting')} data-choice={theme}>{([['light',Sun,'themeLight'],['dark',Moon,'themeDark'],['system',Monitor,'system']] as const).map(([value,Icon,label])=><button key={value} role="radio" aria-checked={theme===value} aria-label={t(label)} title={t(label)} onClick={()=>setTheme(value)}><Icon size={15}/></button>)}</div><div className="core-status"><span className={isDesktop&&coreConnection.phase!=='connected'?'status-dot disconnected':'status-dot'}/><div>{context?.library.name||t('localCore')}<small>{t(isDesktop&&coreConnection.phase!=='connected'?'connection_'+coreConnection.phase:'connected')} · v{APP_VERSION}</small></div></div></div></aside>
 <main id="main-content" tabIndex={-1}><header className={scrolled?'topbar scrolled':'topbar'}><div className="top-actions"><CollectionSearch key={collectionStorageKey('search-history')} storageKey={collectionStorageKey('search-history')} query={query} onChange={value=>{if(view!=='library'||detail)navigate('library');setQuery(value);}}/></div></header>
 {error&&<div className="error-banner" role="alert"><span>{error}</span><button aria-label={t('close')} onClick={()=>setError('')}><X size={16}/></button></div>}
 <div className="content">{identity?.role==='web'&&<p className="read-only-note">{t('readOnlySession')}</p>}

 {view==='home'&&!detail&&<HomePanel generation={collectionGeneration} safe={safe} titleMode={titleMode} unmatched={canWrite?collectionSummary.unassigned:0} activeTasks={active} onSelect={selectWork} onCollection={status=>{navigate('library');if(status)setFilter(status);}} onOrganize={()=>navigate('organize')} onTasks={()=>navigate('tasks')}/>}
 {view==='home'&&detail&&renderLocalWork(detail,backCollection,selectWork,initialSearch,'home')}
 {view==='lists'&&<ListsWorkspace renderLocalWork={renderLocalWork} safe={safe} canWrite={canWrite}/>}
 {view==='library'&&<div hidden={!!detail}><LibraryBrowser generation={collectionGeneration} query={query} setQuery={setQuery} status={filter} setStatus={setFilter} safe={safe} canWrite={canWrite} onAdd={()=>{matchEpoch.current++;setCandidates([]);setMatchInfo(null);setManual(true);setMatchQuery('');setMatcher(null);}} onSource={()=>navigate('sources')} onSelect={selectWork} onStudio={(work,studio)=>{collectionY.current=window.scrollY;setInitialSearch({kind:'studio',value:studio,label:studio});setDetail(work);requestAnimationFrame(()=>window.scrollTo(0,0));}} titleMode={titleMode} setTitleMode={setTitleMode}/></div>}
 {view==='library'&&detail&&renderLocalWork(detail,backCollection,selectWork,initialSearch,'myCollection')}
 {view==='sources'&&<><Heading title={t('sourceTitle')} subtitle={t('sourceSubtitle')} action={canWrite&&<button className="primary" onClick={()=>setNewSource(true)}><Plus size={17}/>{t('addSource')}</button>}/>{roots.length===0&&<div className="empty"><HardDrive size={38}/><p>{t('emptyBody')}</p></div>}{roots.map(r=><div className="source-card" key={r.id}><div className="source-row"><div className="source-icon"><HardDrive size={24}/></div><div><h2>{r.label}</h2><p className="path">{r.path.replace(/^\\\\\?\\/,'')}</p><span className="pill">{t('rootRole_'+r.role,{defaultValue:r.role})}</span> <span className={`pill ${r.online?'':'failed'}`}>{t(r.online?'sourceOnline':'sourceOffline')}</span>{(r.missing>0||r.unverified>0)&&<p>{t('sourceAvailability',{missing:r.missing,unverified:r.unverified})}</p>}</div>{canWrite&&<><button onClick={()=>setRemapping(r)}>{t('remapSource')}</button><button className="primary" onClick={()=>{setScanRoot(r);setScope('');setExcludes('');}}>{t('scan')}<ArrowUpRight size={16}/></button></>}</div><SourceWatch rootId={r.id} readOnly={!canWrite} onScan={exclude=>{setScanRoot(r);setScope('');setExcludes(exclude.join('\n'));}}/></div>)}</>}
 {view==='tasks'&&<><Heading title={t('activityTitle')} subtitle={t('activitySubtitle')}/>{canWrite&&<IssuesPanel onReview={setHistoryResource} onSource={()=>navigate('sources')}/>}{context&&<AutoMatchPanel key={context.library.id} jobs={jobs} libraryId={context.library.id} workCount={collectionSummary.works} canWrite={canWrite} onCollection={()=>navigate('library')} onReview={()=>{navigate('organize');setFilter('unmatched');}} onSources={()=>navigate('sources')}/>}<JobHistory key={'jobs:'+context?.library.id} latest={jobPage} render={(j,changed)=><article className={`job job-${j.state}`} key={j.id}><div className="job-head"><div className="job-title"><span className={`pill ${j.state}`}>{t(j.state)}</span><h2>{t(j.kind==='match'?'job_match':j.reuse_existing?'job_reuse':j.automatic?'automaticScan':'job_'+j.kind)}</h2><time dateTime={new Date(j.created*1000).toISOString()} title={new Date(j.created*1000).toLocaleString(i18n.language)}>{relativeTime(j.created,i18n.language)}</time></div><div className="inline">{canWrite&&j.state==='running'&&<button disabled={busy} onClick={()=>void act(async()=>{await api(`/jobs/${j.id}/pause`,{});await changed();})}><Pause size={15}/>{t('pause')}</button>}{canWrite&&['paused','interrupted','failed','cancelled'].includes(j.state)&&<button disabled={busy} onClick={()=>{if(j.kind==='acquire'){resumeChanged.current=changed;setResuming(j);setPassword('');}else{void act(async()=>{await api(`/jobs/${j.id}/resume`,{});await changed();});}}}><Play size={15}/>{t('resume')}</button>}{canWrite&&['running','queued','paused'].includes(j.state)&&<button disabled={busy} onClick={()=>void act(async()=>{await api(`/jobs/${j.id}/cancel`,{});await changed();})}><Square size={14}/>{t('stop')}</button>}</div></div>{['running','queued','pausing','paused'].includes(j.state)&&<div className={`progress ${j.state==='running'?'indeterminate':''}`} role="progressbar" aria-label={t('taskProgress')} aria-valuemin={0} aria-valuemax={100} aria-valuetext={t('progressUnknown',{processed:j.processed})}>{j.state==='running'&&<span/>}</div>}<div className="job-stats"><span><strong>{j.processed.toLocaleString()}</strong>{t('processed')}</span>{j.kind!=='match'&&<span><strong>{bytes(j.bytes)}</strong>{t(j.kind==='scan'?'bytes':j.kind==='hash'?'hashedSize':'copiedSize')}</span>}<span><strong>{j.discovered.toLocaleString()}</strong>{t('discovered')}</span><span className={j.errors>0?'has-errors':undefined}><strong>{j.errors.toLocaleString()}</strong>{t('errors')}</span></div>{j.summary&&<JobSummary job={j}/>}{['failed','interrupted'].includes(j.state)&&<TaskMessages activity={j.current_path} message={j.message} state={j.state}/>}<details className="job-details"><summary>{t('taskDetails')}</summary>{!j.summary&&<JobSummary job={j}/>}{!['failed','interrupted'].includes(j.state)&&<TaskMessages activity={j.current_path} message={j.message} state={j.state}/>}<p className="path">{t('timelineJob')}: {j.id}</p></details></article>}/>{canWrite&&<Timeline onPlan={setPlan}/>}<PlanHistory plans={plans} updatedPlan={plan} onOpen={setPlan}/></>}
 {view==='organize'&&<><Heading title={t('organizeTitle')} subtitle={t('organizeSubtitle')}/><OrganizePanel status={filter} onStatus={setFilter} generation={collectionGeneration} busy={busy} onHistory={setHistoryResource} onMatch={startMatch} onReferences={setReferenceResource} onGroup={setRegroupResource} onPreview={r=>void act(()=>pickPlan(r))}/></>}
 {view==='quarantine'&&<><Heading title={t('quarantineTitle')} subtitle={t('quarantineSubtitle')}/><DuplicatePanel roots={roots} jobs={jobs} onStarted={refresh} onPlan={setPlan} onActivity={()=>navigate('tasks')}/><h2>{t('quarantine')}</h2>{isolated.length===0?<p className="muted">{t('noQuarantine')}</p>:isolated.map(q=><div className="resource-row" key={q.id}><Archive size={20}/><div><strong>{q.original.split(/[\\/]/).pop()}</strong><p className="path">{q.original}</p></div><span className="pill">{q.state}</span>{q.state==='isolated'&&<button onClick={()=>void act(async()=>setPlan(await api(`/quarantine/${q.id}/restore`,{})))}>{t('restore')}</button>}</div>)}</>}
 {view==='settings'&&<><Heading title={t('settings')} subtitle={`Galroon · ${APP_VERSION}`}/><SettingsLayout
 appearance={<><section className="settings-row"><Globe size={22}/><div><h2>{t('language')}</h2><p>{t('languageHint')}</p></div><select aria-label={t('language')} value={localStorage.getItem('galroon.language')||'system'} onChange={e=>{if(e.target.value==='system')localStorage.removeItem('galroon.language');else localStorage.setItem('galroon.language',e.target.value);void i18n.changeLanguage(resolveLanguage(e.target.value==='system'?navigator.language:e.target.value));}}><option value="system">{t('system')}</option><option value="en">English</option><option value="zh-TW">繁體中文</option></select></section>
 <section className="settings-row"><SunMoon size={22}/><div><h2>{t('themeSetting')}</h2><p>{t('themeHint')}</p></div><select aria-label={t('themeSetting')} value={theme} onChange={e=>setTheme(e.target.value as ThemeChoice)}><option value="system">{t('system')}</option><option value="dark">{t('themeDark')}</option><option value="light">{t('themeLight')}</option></select></section>
 <section className="settings-row"><Languages size={22}/><div><h2>{t('nameDisplay')}</h2><p>{t('nameDisplayHint')}</p></div><select aria-label={t('nameDisplay')} value={nameMode} onChange={e=>setNameMode(e.target.value as NameMode)}><option value="romanized">{t('nameRomanized')}</option><option value="original">{t('nameOriginal')}</option></select></section>
 <section className="settings-row"><BookOpen size={22}/><div><h2>{t('titleDisplay')}</h2><p>{t('titleDisplayHint')}</p></div><select aria-label={t('titleDisplay')} value={titleMode} onChange={e=>setTitleMode(e.target.value as 'display'|'original')}><option value="display">{t('displayTitle')}</option><option value="original">{t('originalTitleChoice')}</option></select></section>
 <section className="settings-row"><EyeOff size={22}/><div><h2>{t('safeMode')}</h2><p>{t('safeHint')}</p></div><input type="checkbox" role="switch" checked={safe} aria-label={t('safeMode')} onChange={e=>{setSafe(e.target.checked);localStorage.setItem('galroon.safe',String(e.target.checked));}}/></section></>}
 collection={<>{identity?.can_manage_access&&<ArtworkCachePanel/>}{isDesktop&&<ConnectionsPanel/>}{context&&<CollectionContextPanel key={context.library.id+':'+context.library.revision} context={context} onChanged={refresh}/>}{isDesktop&&identity?.can_manage_access&&<section className="settings-row"><HardDrive size={22}/><div><h2>{t('backgroundCore')}</h2><p>{t('backgroundCoreHint')}</p></div><button disabled={busy} onClick={()=>void act(stopCore)}>{t('stopCore')}</button></section>}</>}
 account={<>{identity?.can_manage_access?<AccessPanel/>:<p>{t('settingsAccessManaged')}</p>}{!isDesktop&&<DiagnosticsPanel core={false}/>} {!isDesktop&&<button onClick={()=>void signOut().catch(reportError).finally(()=>{setReady(false);setIdentity(null);setDetail(null);})}>{t('signOut')}</button>}</>}
 backup={canWrite?<><BackupPanel allowRestore={identity?.can_manage_access===true} onRestored={async()=>{window.location.reload();}}/>{isDesktop&&identity?.can_manage_access&&<DiagnosticsPanel/>}</>:null}
 /></>}
 </div></main>
 {(newSource||scanRoot||matcher||manual||plan||acquiring||resuming)&&<div className="scrim"><aside className="drawer"><button className="drawer-close icon-button" aria-label={t('close')} onClick={()=>{matchEpoch.current++;setNewSource(false);setScanRoot(null);setMatcher(null);setManual(false);setPlan(null);setAcquiring(null);setResuming(null);setPassword('');}}><X/></button>
 {resuming&&<><h2>{t('resume')}</h2><p>{t('passwordHint')}</p><label>{t('password')}<input type="password" value={password} onChange={e=>setPassword(e.target.value)} autoComplete="off"/></label><footer><button className="primary" disabled={busy} onClick={()=>void act(async()=>{await api(`/jobs/${resuming.id}/resume`,{password:password||undefined});await resumeChanged.current();setPassword('');setResuming(null);})}>{t('resume')}</button></footer></>}
 {acquiring&&<AcquirePanel deviceName={context?.core.device.name} key={acquiring.id} resource={acquiring} onStarted={()=>{setAcquiring(null);navigate('tasks');void refresh().catch(reportError);}}/>}
 {newSource&&<><div className="eyebrow">{t('eyebrowSource')}</div><h2>{t('addSource')}</h2><p>{t('sourceSubtitle')}</p><label>{t('sourceLabel')}<input value={label} onChange={e=>setLabel(e.target.value)}/></label><label>{t('sourcePath')}<textarea value={path} onChange={e=>setPath(e.target.value)}/></label><button onClick={()=>void act(async()=>{const p=await folder();if(p)setPath(p);})}><FolderOpen size={16}/>{t('browse')}</button><footer><button className="primary" disabled={busy||!path} onClick={()=>void act(async()=>{await api('/roots',{path,label});setNewSource(false);setPath('');setLabel('');})}>{t('save')}</button></footer></>}
 {scanRoot&&<><div className="eyebrow">{t('eyebrowScan')}</div><h2>{t('scan')}</h2><p className="path">{scanRoot.path}</p><label>{t('scope')}<input placeholder={t('scopeHint')} value={scope} onChange={e=>setScope(e.target.value)}/></label><label>{t('exclude')}<textarea placeholder={t('excludeHint')} value={excludes} onChange={e=>setExcludes(e.target.value)}/></label><footer><button className="primary" disabled={busy} onClick={()=>void act(async()=>{await api('/scans',{root_id:scanRoot.id,scope,exclude:excludes.split('\n').map(x=>x.trim()).filter(Boolean)});setScanRoot(null);navigate('tasks');})}><Play size={16}/>{t('scan')}</button></footer></>}
 {(matcher||manual)&&<><div className="eyebrow">{t('eyebrowMatch')}</div><h2>{t('matchTitle')}</h2>{matcher&&<p className="path">{matcher.path}</p>}<label>{t('query')}<input value={matchQuery} onChange={e=>{matchEpoch.current++;setMatchQuery(e.target.value);setCandidates([]);setMatchInfo(null);}}/></label>{matcher&&<label>{t('edition')}<input value={edition} onChange={e=>setEdition(e.target.value)}/></label>}<div className="inline"><button disabled={busy||!matchQuery} className="primary" onClick={()=>void act(async()=>{const epoch=++matchEpoch.current;setCandidates([]);setMatchInfo(null);const data=await api<{results:Candidate[];searched:string[];incomplete:boolean;warning?:string;needs_title?:boolean}>('/vndb/search',{query:matchQuery,...(matcher&&matchQuery===matcher.title?{resource_id:matcher.id}:{})});if(epoch===matchEpoch.current){setCandidates(data.results);setMatchInfo(data);}})}><Search size={15}/>{t('searchVndb')}</button><button disabled={busy||!matchQuery} onClick={()=>void act(()=>createCandidate())}>{t('manual')}</button></div>{matchInfo&&<div className="match-evidence" role="status"><p>{t('matchReviewHint')}</p>{matchInfo.searched.length>0&&<p>{t('matchSearched')}: {matchInfo.searched.join(' · ')}</p>}{matchInfo.needs_title&&<p>{t('matchNeedsTitle')}</p>}{!matchInfo.needs_title&&candidates.length===0&&<p>{t('matchNoResults')}</p>}{matchInfo.incomplete&&<p>{t('matchIncomplete')}</p>}{matchInfo.warning&&<p className="error-text">{matchInfo.warning}</p>}</div>}{candidates.map(c=><button className="candidate" disabled={busy} key={c.id} onClick={()=>void act(()=>createCandidate(c))}>{c.image?.url&&!safe&&<CachedImage source={c.image.url}/>}<span><strong>{c.title}</strong><small>{c.alttitle}</small><small>{c.id} · {c.released}</small>{c.match&&<><small className="match-reason">{t(`matchReason_${c.match.reason}`)}{c.match.matched_title?`: ${c.match.matched_title}`:''}</small><small>{t(c.match.ambiguous?'matchAmbiguous':c.match.strength==='strong'?'matchStrong':'matchReview')}</small></>}</span><ChevronRight size={18}/></button>)}{matcher&&<MatchWorkChoices query={matchQuery} generation={collectionGeneration} busy={busy} onChoose={id=>void act(()=>chooseWork(id))}/>}</>}
 {plan&&<PlanReview key={plan.id} plan={plan} canWrite={canWrite} onChanged={p=>{setPlan(p);void refresh().catch(reportError);}}/>}
 {busy&&<p className="busy-note" role="status"><RefreshCw size={14} className="spin"/>{t('loading')}</p>}{error&&<p className="error-text" role="alert">{error}</p>}</aside></div>}
 {historyResource&&<MatchHistory resource={historyResource} onClose={()=>setHistoryResource(null)}/>} 
 {regroupResource&&<ResourceGrouping resource={regroupResource} onClose={()=>setRegroupResource(null)} onSaved={refresh}/>}
 {referenceResource&&<ResourceReferences resource={referenceResource} onClose={()=>setReferenceResource(null)} onSaved={refresh}/>}
 {remapping&&<SourceRemap root={remapping} onClose={()=>setRemapping(null)} onSaved={refresh}/>}
 {groupingWork&&<GroupingEditor work={groupingWork} onClose={()=>setGroupingWork(null)} onSaved={refresh}/>}
 {editingMetadata&&<MetadataEditor work={editingMetadata} onClose={()=>setEditingMetadata(null)} onSaved={()=>reloadWork(editingMetadata.id)}/>}
 </div></></CatalogGenerationContext.Provider></DisplayContext.Provider>;
}
/** Short, human timestamps for activity; the exact time stays in the tooltip. */
function relativeTime(seconds:number,locale:string){const diff=seconds-Date.now()/1000,abs=Math.abs(diff);const format=new Intl.RelativeTimeFormat(locale,{numeric:'auto'});if(abs<60)return format.format(0,'second');if(abs<3600)return format.format(Math.round(diff/60),'minute');if(abs<86400)return format.format(Math.round(diff/3600),'hour');if(abs<86400*30)return format.format(Math.round(diff/86400),'day');return new Date(seconds*1000).toLocaleDateString(locale);}
function Heading({title,subtitle,action}:{title:string;subtitle:string;action?:React.ReactNode}){return <section className="page-heading"><div><h1>{title}</h1><p>{subtitle}</p></div>{action}</section>}
const container=document.getElementById('root')! as HTMLElement & {__galroonRoot?:ReturnType<typeof createRoot>;__galroonDrawerKeys?:()=>void};
container.__galroonDrawerKeys??=installDrawerKeys();
const appRoot=container.__galroonRoot??=createRoot(container);
appRoot.render(<React.StrictMode><App/></React.StrictMode>);


















