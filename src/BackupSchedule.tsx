import {useCallback,useEffect,useRef,useState,type CSSProperties} from 'react';
import {useTranslation} from 'react-i18next';
import {AlertTriangle,Clock,FolderOpen,Play,RefreshCw,ShieldAlert} from 'lucide-react';
import {api,bytes,folder,isLocalDesktop} from './api';

export type BackupMode='portable'|'full';
export type BackupOrigin='manual'|'scheduled';
export type BackupFrequency='daily'|'weekly';

export type BackupScheduleSettings={
 enabled:boolean;
 destination:string;
 frequency:BackupFrequency;
 minute_utc:number;
 weekday:number|null;
 keep:number;
 mode:BackupMode;
 artwork:boolean;
};

export type BackupScheduleState=BackupScheduleSettings&{
 version:number;
 last_started:number|null;
 last_success:number|null;
 last_error:string|null;
 last_path:string|null;
 running:boolean;
 next_run:number|null;
 available_bytes:number|null;
};

type ScheduleDraft={
 enabled:boolean;
 destination:string;
 frequency:BackupFrequency;
 localTime:string;
 localWeekday:number;
 keep:string;
 mode:BackupMode;
 artwork:boolean;
};

const DAY_SECONDS=86400;
const DAY_MINUTES=1440;

export function formatMinute(minute:number):string{
 const clamped=((Math.trunc(minute)%DAY_MINUTES)+DAY_MINUTES)%DAY_MINUTES;
 const h=Math.floor(clamped/60);
 const m=clamped%60;
 return `${String(h).padStart(2,'0')}:${String(m).padStart(2,'0')}`;
}

export function parseTimeInput(value:string):number|null{
 const match=/^(\d{1,2}):(\d{2})$/.exec(value.trim());
 if(!match)return null;
 const h=Number(match[1]),m=Number(match[2]);
 if(!Number.isInteger(h)||!Number.isInteger(m)||h<0||h>23||m<0||m>59)return null;
 return h*60+m;
}

/** Convert UTC minute (0..1439) and optional UTC weekday (0=Sun..6=Sat) to local time.
 *  `offsetMinutes` follows `Date.prototype.getTimezoneOffset()` (UTC - local, e.g. -480 for UTC+8). */
export function utcToLocalSchedule(minuteUtc:number,weekdayUtc:number|null,offsetMinutes:number=new Date().getTimezoneOffset()){
 const normalizedUtc=((Math.trunc(minuteUtc)%DAY_MINUTES)+DAY_MINUTES)%DAY_MINUTES;
 const localTotal=normalizedUtc-offsetMinutes;
 const dayShift=Math.floor(localTotal/DAY_MINUTES);
 const localMinute=((localTotal%DAY_MINUTES)+DAY_MINUTES)%DAY_MINUTES;
 const localWeekday=weekdayUtc===null?null:(((weekdayUtc+dayShift)%7)+7)%7;
 return {localMinute,localTime:formatMinute(localMinute),localWeekday,utcTime:formatMinute(normalizedUtc),dayShift};
}

/** Convert local minute (0..1439) and optional local weekday (0=Sun..6=Sat) to UTC for Core.
 *  `offsetMinutes` follows `Date.prototype.getTimezoneOffset()` (UTC - local, e.g. -480 for UTC+8). */
export function localToUtcSchedule(localMinute:number,localWeekday:number|null,offsetMinutes:number=new Date().getTimezoneOffset()){
 const normalizedLocal=((Math.trunc(localMinute)%DAY_MINUTES)+DAY_MINUTES)%DAY_MINUTES;
 const utcTotal=normalizedLocal+offsetMinutes;
 const dayShift=Math.floor(utcTotal/DAY_MINUTES);
 const minuteUtc=((utcTotal%DAY_MINUTES)+DAY_MINUTES)%DAY_MINUTES;
 const weekdayUtc=localWeekday===null?null:(((localWeekday+dayShift)%7)+7)%7;
 return {minuteUtc,weekdayUtc,utcTime:formatMinute(minuteUtc),dayShift};
}

/** Overdue when enabled and more than two periods have elapsed since the last successful backup. */
export function isScheduleOverdue(
 schedule:Pick<BackupScheduleState,'enabled'|'frequency'|'last_success'>,
 nowSeconds:number=Math.floor(Date.now()/1000),
):boolean{
 if(!schedule.enabled||schedule.last_success===null)return false;
 const period=schedule.frequency==='weekly'?7*DAY_SECONDS:DAY_SECONDS;
 return nowSeconds-schedule.last_success>2*period;
}

const bannerWarnStyle:CSSProperties={
 display:'flex',alignItems:'flex-start',gap:10,padding:'12px 14px',margin:'12px 0',
 borderRadius:'var(--radius-s)',background:'var(--warn-soft)',border:'1px solid var(--warn-line)',
 color:'var(--warn)',fontSize:13,lineHeight:1.5,maxWidth:'100%',overflowWrap:'anywhere',
};

const cardStyle:CSSProperties={
 padding:'16px 18px',margin:'16px 0',borderRadius:'var(--radius)',
 background:'var(--surface)',border:'.5px solid var(--line)',maxWidth:'100%',minWidth:0,
};

const gridStyle:CSSProperties={
 display:'grid',gridTemplateColumns:'repeat(auto-fit,minmax(min(100%,160px),1fr))',gap:'12px 16px',margin:'12px 0',
};

const statusGridStyle:CSSProperties={
 display:'grid',gridTemplateColumns:'repeat(auto-fit,minmax(min(100%,150px),1fr))',gap:'10px 16px',margin:'12px 0',
};

const fullWidthInputStyle:CSSProperties={width:'100%',minWidth:0,boxSizing:'border-box'};

export function BackupFullWarning(){
 const {t}=useTranslation();
 return <div className="backup-full-warning" role="note" data-testid="backup-full-warning" style={bannerWarnStyle}>
  <ShieldAlert size={18} style={{flexShrink:0,marginTop:2}} aria-hidden="true"/>
  <span style={{minWidth:0}}>{t('backupFullWarning')}</span>
 </div>;
}

export function BackupOverdueBanner(){
 const {t}=useTranslation();
 return <div className="backup-overdue-banner" role="alert" data-testid="backup-overdue-banner" style={bannerWarnStyle}>
  <AlertTriangle size={18} style={{flexShrink:0,marginTop:2}} aria-hidden="true"/>
  <span style={{minWidth:0}}>{t('backupOverdueWarning')}</span>
 </div>;
}

function toDraft(state:BackupScheduleState,isOwner:boolean,offsetMinutes?:number):ScheduleDraft{
 const local=utcToLocalSchedule(state.minute_utc,state.weekday,offsetMinutes);
 const mode:BackupMode=!isOwner&&!state.enabled&&state.mode==='full'?'portable':state.mode;
 return {
  enabled:state.enabled,
  destination:state.destination,
  frequency:state.frequency,
  localTime:local.localTime,
  localWeekday:local.localWeekday??0,
  keep:String(state.keep),
  mode,
  artwork:state.artwork,
 };
}

export interface BackupScheduleProps{
 isOwner:boolean;
 onChanged?:()=>Promise<void>|void;
 refreshToken?:number;
 initialSchedule?:BackupScheduleState|null;
 timezoneOffsetMinutes?:number;
 nowSeconds?:number;
}

export function BackupSchedule({
 isOwner,
 onChanged,
 refreshToken=0,
 initialSchedule=null,
 timezoneOffsetMinutes,
 nowSeconds,
}:BackupScheduleProps){
 const {t,i18n}=useTranslation();
 const [schedule,setSchedule]=useState<BackupScheduleState|null>(initialSchedule);
 const [draft,setDraft]=useState<ScheduleDraft|null>(()=>(initialSchedule?toDraft(initialSchedule,isOwner,timezoneOffsetMinutes):null));
 const [dirty,setDirty]=useState(false);
 const [busy,setBusy]=useState(false);
 const [message,setMessage]=useState('');
 const [error,setError]=useState('');
 const wasRunning=useRef(initialSchedule?.running===true);
 const canBrowseLocal=isLocalDesktop();

 const applySchedule=useCallback((next:BackupScheduleState,forceDraft:boolean)=>{
  setSchedule(next);
  if(forceDraft||!dirty){setDraft(toDraft(next,isOwner,timezoneOffsetMinutes));if(forceDraft)setDirty(false);}
 },[dirty,isOwner,timezoneOffsetMinutes]);

 const load=useCallback(async(forceDraft=false)=>{
  try{
   const next=await api<BackupScheduleState>('/backups/schedule');
   if(wasRunning.current&&!next.running){void onChanged?.();}
   wasRunning.current=next.running;
   applySchedule(next,forceDraft);
   setError('');
  }catch(e){
   setError(e instanceof Error?e.message:String(e));
  }
 },[applySchedule,onChanged]);

 useEffect(()=>{void load(schedule===null);},[refreshToken]);

 useEffect(()=>{
  if(!schedule?.running)return;
  const timer=setInterval(()=>{void load(false);},5000);
  return ()=>clearInterval(timer);
 },[schedule?.running,load]);

 const act=async(work:()=>Promise<void>)=>{
  setBusy(true);setError('');setMessage('');
  try{await work();}
  catch(e){setError(e instanceof Error?e.message:String(e));}
  finally{setBusy(false);}
 };

 const updateDraft=(patch:Partial<ScheduleDraft>)=>{
  setDraft(prev=>prev?{...prev,...patch}:prev);
  setDirty(true);
 };

 const parsedMinute=draft?parseTimeInput(draft.localTime):null;
 const utcPreview=draft&&parsedMinute!==null
  ?localToUtcSchedule(parsedMinute,draft.frequency==='weekly'?draft.localWeekday:null,timezoneOffsetMinutes)
  :null;
 const keepNum=draft?Number(draft.keep):NaN;
 const keepValid=Number.isInteger(keepNum)&&keepNum>=1&&keepNum<=100;
 const modeForbidden=!isOwner&&draft?.mode==='full';
 const runForbidden=!isOwner&&schedule?.mode==='full';
 const canSave=!!draft&&!busy&&!schedule?.running&&draft.destination.trim().length>0&&parsedMinute!==null&&keepValid&&!modeForbidden;
 const canRun=!!schedule&&!busy&&!schedule.running&&schedule.destination.trim().length>0&&!runForbidden;
 const overdue=schedule?isScheduleOverdue(schedule,nowSeconds):false;

 const formatTimestamp=(ts:number|null,emptyKey:string)=>
  ts===null?t(emptyKey):new Date(ts*1000).toLocaleString(i18n.resolvedLanguage);

 const save=()=>void act(async()=>{
  if(!draft||parsedMinute===null||!keepValid)return;
  if(!isOwner&&draft.mode==='full'){setError(t('backupFullOwnerOnly'));return;}
  const converted=localToUtcSchedule(parsedMinute,draft.frequency==='weekly'?draft.localWeekday:null,timezoneOffsetMinutes);
  const payload:BackupScheduleSettings={
   enabled:draft.enabled,
   destination:draft.destination.trim(),
   frequency:draft.frequency,
   minute_utc:converted.minuteUtc,
   weekday:draft.frequency==='weekly'?(converted.weekdayUtc??0):null,
   keep:keepNum,
   mode:draft.mode,
   artwork:draft.artwork,
  };
  const updated=await api<BackupScheduleState>('/backups/schedule',payload);
  wasRunning.current=updated.running;
  setSchedule(updated);
  setDraft(toDraft(updated,isOwner,timezoneOffsetMinutes));
  setDirty(false);
  setMessage(t('backupScheduleSaved'));
 });

 const runNow=()=>void act(async()=>{
  if(!isOwner&&schedule?.mode==='full'){setError(t('backupFullOwnerOnly'));return;}
  const result=await api<{deferred:boolean;path:string|null}>('/backups/run',{});
  await load(false);
  await onChanged?.();
  if(result.deferred)setMessage(t('backupRunDeferred'));
  else if(result.path)setMessage(t('backupSaved',{path:result.path}));
 });

 return <div className="backup-schedule" style={{minWidth:0,maxWidth:'100%'}}>
  {overdue&&<BackupOverdueBanner/>}
  <div style={cardStyle}>
   <div className="inline" style={{justifyContent:'space-between',alignItems:'center'}}>
    <h3 style={{margin:0,display:'inline-flex',alignItems:'center',gap:8}}><Clock size={18} aria-hidden="true"/> {t('backupScheduleTitle')}</h3>
    <div className="inline">
     {schedule&&(schedule.running
      ?<span className="pill running" data-testid="backup-running-pill">{t('backupScheduleRunning')}</span>
      :<span className={`pill ${schedule.enabled?'completed':'paused'}`}>{t(schedule.enabled?'backupScheduleActive':'backupSchedulePaused')}</span>)}
     <button type="button" disabled={busy} onClick={()=>void act(()=>load(false))} aria-label={t('backupScheduleRefresh')}><RefreshCw size={14} className={schedule?.running?'spin':undefined}/>{t('backupScheduleRefresh')}</button>
    </div>
   </div>
   <p className="muted" style={{margin:'8px 0 0'}}>{t('backupScheduleHint')}</p>

   {schedule&&<dl style={statusGridStyle}>
    <div style={{minWidth:0}}><dt className="muted" style={{fontSize:12}}>{t('backupLastSuccess')}</dt><dd style={{fontSize:13,overflowWrap:'anywhere'}}>{formatTimestamp(schedule.last_success,'backupNever')}</dd></div>
    <div style={{minWidth:0}}><dt className="muted" style={{fontSize:12}}>{t('backupNextRun')}</dt><dd style={{fontSize:13,overflowWrap:'anywhere'}}>{formatTimestamp(schedule.next_run,'backupScheduleNone')}</dd></div>
    <div style={{minWidth:0}}><dt className="muted" style={{fontSize:12}}>{t('backupFreeSpace')}</dt><dd style={{fontSize:13,overflowWrap:'anywhere'}}>{schedule.available_bytes!==null?bytes(schedule.available_bytes):t('unknown')}</dd></div>
   </dl>}

   {schedule?.last_error&&<p className="error-text" role="alert" style={{overflowWrap:'anywhere',margin:'8px 0'}}>{t('backupLastError')}: {schedule.last_error}</p>}

   {!draft&&!error&&<p className="muted" role="status">{t('loading')}</p>}

   {draft&&<div style={{marginTop:14,minWidth:0}}>
    <label className="checkbox-label" style={{marginBottom:12}}>
     <input type="checkbox" role="switch" checked={draft.enabled} disabled={busy} onChange={e=>updateDraft({enabled:e.target.checked})}/>
     <span>{t('backupScheduleEnabled')}</span>
    </label>

    <label style={{marginBottom:8,minWidth:0}}>
     {t('backupScheduleDestination')}
     <input style={fullWidthInputStyle} placeholder="/backups" aria-describedby="backup-schedule-destination-hint" value={draft.destination} disabled={busy} onChange={e=>updateDraft({destination:e.target.value})}/>
    </label>
    <p id="backup-schedule-destination-hint" className="muted" style={{margin:'0 0 10px',fontSize:12.5}}>{t('backupScheduleDestinationHint')}</p>
    {canBrowseLocal&&<div className="inline" style={{marginBottom:12}}>
     <button type="button" disabled={busy} onClick={()=>void act(async()=>{const p=await folder();if(p)updateDraft({destination:p});})}><FolderOpen size={16}/>{t('browse')}</button>
    </div>}

    <div style={gridStyle}>
     <label style={{minWidth:0}}>
      {t('backupScheduleFrequency')}
      <select style={fullWidthInputStyle} value={draft.frequency} disabled={busy} onChange={e=>updateDraft({frequency:e.target.value as BackupFrequency})}>
       <option value="daily">{t('backupScheduleDaily')}</option>
       <option value="weekly">{t('backupScheduleWeekly')}</option>
      </select>
     </label>

     {draft.frequency==='weekly'&&<label style={{minWidth:0}}>
      {t('backupScheduleWeekday')}
      <select style={fullWidthInputStyle} value={draft.localWeekday} disabled={busy} onChange={e=>updateDraft({localWeekday:Number(e.target.value)})}>
       {[0,1,2,3,4,5,6].map(day=><option key={day} value={day}>{t(`backupWeekday_${day}`)}</option>)}
      </select>
     </label>}

     <label style={{minWidth:0}}>
      {t('backupScheduleTime')}
      <input style={fullWidthInputStyle} type="time" value={draft.localTime} disabled={busy} onChange={e=>updateDraft({localTime:e.target.value})}/>
     </label>

     <label style={{minWidth:0}}>
      {t('backupScheduleKeep')}
      <input style={fullWidthInputStyle} type="number" min={1} max={100} step={1} value={draft.keep} disabled={busy} onChange={e=>updateDraft({keep:e.target.value})}/>
     </label>

     <label style={{minWidth:0}}>
      {t('backupScheduleMode')}
      <select style={fullWidthInputStyle} value={draft.mode} disabled={busy} onChange={e=>{
       const next=e.target.value as BackupMode;
       if(next==='full'&&!isOwner)return;
       updateDraft({mode:next});
      }}>
       <option value="full" disabled={!isOwner}>{t('backupMode_full')}</option>
       <option value="portable">{t('backupMode_portable')}</option>
      </select>
     </label>
    </div>

    {utcPreview&&parsedMinute!==null&&<p className="muted" data-testid="backup-time-display" style={{margin:'0 0 10px',fontSize:12.5}}>
     {draft.frequency==='weekly'
      ?t('backupScheduleTimeDisplayWeekly',{localDay:t(`backupWeekday_${draft.localWeekday}`),local:formatMinute(parsedMinute),utcDay:t(`backupWeekday_${utcPreview.weekdayUtc??0}`),utc:utcPreview.utcTime})
      :t('backupScheduleTimeDisplay',{local:formatMinute(parsedMinute),utc:utcPreview.utcTime})}
    </p>}

    {!isOwner&&<p className="muted" data-testid="backup-owner-only-hint" style={{margin:'0 0 10px',fontSize:12.5}}>{t('backupFullOwnerOnly')}</p>}

    {draft.mode==='full'&&<BackupFullWarning/>}

    <label className="checkbox-label" style={{margin:'12px 0'}}>
     <input type="checkbox" checked={draft.artwork} disabled={busy} onChange={e=>updateDraft({artwork:e.target.checked})}/>
     <span>{t('backupScheduleArtwork')}</span>
    </label>

    <div className="inline" style={{marginTop:12}}>
     <button type="button" className="primary" disabled={!canSave} onClick={save}>{t('backupScheduleSave')}</button>
     <button type="button" disabled={!canRun} onClick={runNow}><Play size={15}/>{t('backupRunNow')}</button>
    </div>
   </div>}

   {busy&&<p role="status" style={{marginTop:10}}>{t('loading')}</p>}
   {message&&<p className="path" role="status" style={{marginTop:10}}>{message}</p>}
   {error&&<p className="error-text" role="alert" style={{marginTop:10}}>{error}</p>}
  </div>
 </div>;
}
