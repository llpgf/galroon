import {beforeEach,describe,expect,it,vi} from 'vitest';
import {isValidElement,type ReactElement,type ReactNode} from 'react';
import {renderToStaticMarkup} from 'react-dom/server';

const hooks=vi.hoisted(()=>{
 vi.stubGlobal('localStorage',{getItem:()=>null,setItem:()=>{},removeItem:()=>{}});
 vi.stubGlobal('navigator',{language:'en'});
 vi.stubGlobal('document',{documentElement:{lang:''}});
 return {
  intercept:false,
  values:[] as unknown[],
  refs:[] as {current:unknown}[],
  effects:[] as (()=>void|(()=>void))[],
  cursor:0,
  refCursor:0,
 };
});

const apiMock=vi.hoisted(()=>({
 api:vi.fn(),
 deleteApi:vi.fn(),
 folder:vi.fn(),
 restoreCore:vi.fn(),
 isLocalDesktop:vi.fn(()=>false),
 isDesktop:false,
}));

vi.mock('react',async original=>{
 const actual=await original<typeof import('react')>();
 return {
  ...actual,
  useState:(initial:unknown)=>{
   if(!hooks.intercept)return actual.useState(initial);
   const i=hooks.cursor++;
   if(!(i in hooks.values))hooks.values[i]=typeof initial==='function'?(initial as ()=>unknown)():initial;
   return [hooks.values[i],(value:unknown)=>{hooks.values[i]=typeof value==='function'?(value as (prev:unknown)=>unknown)(hooks.values[i]):value;}];
  },
  useEffect:(effect:()=>void|(()=>void))=>{
   if(!hooks.intercept)return actual.useEffect(effect);
   hooks.effects.push(effect);
  },
  useCallback:<T extends (...args:never[])=>unknown>(fn:T,deps:unknown[])=>{
   if(!hooks.intercept)return actual.useCallback(fn,deps);
   return fn;
  },
  useRef:<T,>(initial:T)=>{
   if(!hooks.intercept)return actual.useRef(initial);
   const i=hooks.refCursor++;
   if(!(i in hooks.refs))hooks.refs[i]={current:initial};
   return hooks.refs[i];
  },
 };
});

vi.mock('react-i18next',async original=>{
 const actual=await original<typeof import('react-i18next')>();
 return {
  ...actual,
  useTranslation:()=>{
   if(!hooks.intercept)return actual.useTranslation();
   return {t:i18n.t.bind(i18n),i18n,ready:true};
  },
 };
});

vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn().mockResolvedValue([]),isTauri:()=>false}));

vi.mock('./api',async original=>{
 const actual=await original<typeof import('./api')>();
 return {
  ...actual,
  api:apiMock.api,
  deleteApi:apiMock.deleteApi,
  folder:apiMock.folder,
  restoreCore:apiMock.restoreCore,
  isLocalDesktop:()=>apiMock.isLocalDesktop(),
  get isDesktop(){return apiMock.isDesktop;},
 };
});

import i18n from './i18n';
import {
 BackupSchedule,
 formatMinute,
 isScheduleOverdue,
 localToUtcSchedule,
 parseTimeInput,
 utcToLocalSchedule,
 type BackupScheduleState,
} from './BackupSchedule';
import {BackupPanel,type BackupRecord} from './BackupPanel';

type Element=ReactElement<Record<string,any>>;

function nodes(node:ReactNode):Element[]{
 if(Array.isArray(node))return node.flatMap(nodes);
 if(!isValidElement(node))return [];
 const el=node as Element;
 return [el,...nodes(el.props.children)];
}

function textOf(node:ReactNode):string{
 if(node===null||node===undefined||typeof node==='boolean')return '';
 if(typeof node==='string'||typeof node==='number')return String(node);
 if(Array.isArray(node))return node.map(textOf).join('');
 if(isValidElement(node))return textOf((node as Element).props.children);
 return '';
}

function resetHooks(intercept:boolean){
 hooks.intercept=intercept;
 hooks.values=[];
 hooks.refs=[];
 hooks.effects=[];
 hooks.cursor=0;
 hooks.refCursor=0;
}

async function settle(){for(let i=0;i<10;i++)await Promise.resolve();}

const DAY=86400;
const NOW=1_760_600_000;

function makeSchedule(patch:Partial<BackupScheduleState>={}):BackupScheduleState{
 return {
  version:1,
  enabled:true,
  destination:'/backups',
  frequency:'daily',
  minute_utc:1230, // 20:30 UTC -> 04:30 UTC+8
  weekday:null,
  keep:14,
  mode:'full',
  artwork:true,
  last_started:NOW-3600,
  last_success:NOW-3600,
  last_error:null,
  last_path:'/backups/Galroon-backup-1',
  running:false,
  next_run:NOW+82800,
  available_bytes:10*1024**3,
  ...patch,
 };
}

function makeRecord(patch:Partial<BackupRecord>={}):BackupRecord{
 return {
  id:'b-manual-1',
  path:'/backups/Galroon-backup-100',
  created:NOW-1000,
  origin:'manual',
  mode:'portable',
  deleted:false,
  manifest:{
   created:NOW-1000,
   bytes:1024*1024,
   sha256:'abc123',
   version:4,
   mode:'portable',
   origin:'manual',
   library:{id:'lib-1',name:'My Collection'},
   schema:52,
  },
  ...patch,
 };
}

beforeEach(async()=>{
 resetHooks(false);
 apiMock.api.mockReset();
 apiMock.deleteApi.mockReset();
 apiMock.folder.mockReset();
 apiMock.restoreCore.mockReset();
 apiMock.isLocalDesktop.mockReturnValue(false);
 apiMock.isDesktop=false;
 await i18n.changeLanguage('en');
});

describe('UTC / local time conversion',()=>{
 it('converts daily UTC minute to local time and back across midnight (UTC+8)',()=>{
  // UTC+8 => getTimezoneOffset() === -480
  const local=utcToLocalSchedule(1230,null,-480); // 20:30 UTC
  expect(local.localTime).toBe('04:30');
  expect(local.utcTime).toBe('20:30');
  expect(local.dayShift).toBe(1);
  const back=localToUtcSchedule(local.localMinute,null,-480);
  expect(back.minuteUtc).toBe(1230);
  expect(back.utcTime).toBe('20:30');
  expect(back.dayShift).toBe(-1);
 });

 it('converts weekly weekday across midnight in both positive and negative offsets',()=>{
  // UTC+8: Sunday 20:30 UTC (weekday=0) === Monday 04:30 local (weekday=1)
  const taipei=utcToLocalSchedule(1230,0,-480);
  expect(taipei.localTime).toBe('04:30');
  expect(taipei.localWeekday).toBe(1);
  const taipeiBack=localToUtcSchedule(taipei.localMinute,taipei.localWeekday,-480);
  expect(taipeiBack.minuteUtc).toBe(1230);
  expect(taipeiBack.weekdayUtc).toBe(0);

  // UTC-5 (offset +300): Sunday 22:00 local (weekday=0) === Monday 03:00 UTC (weekday=1)
  const nyUtc=localToUtcSchedule(22*60,0,300);
  expect(nyUtc.minuteUtc).toBe(180);
  expect(nyUtc.utcTime).toBe('03:00');
  expect(nyUtc.weekdayUtc).toBe(1);
  const nyLocal=utcToLocalSchedule(nyUtc.minuteUtc,nyUtc.weekdayUtc,300);
  expect(nyLocal.localTime).toBe('22:00');
  expect(nyLocal.localWeekday).toBe(0);

  // UTC+5:30 (offset -330): 22:30 UTC (1350) === 04:00 local (+1 day)
  const india=utcToLocalSchedule(1350,6,-330);
  expect(india.localTime).toBe('04:00');
  expect(india.localWeekday).toBe(0);
  expect(localToUtcSchedule(india.localMinute,india.localWeekday,-330)).toMatchObject({minuteUtc:1350,weekdayUtc:6});
 });

 it('validates and formats HH:MM time inputs',()=>{
  expect(formatMinute(0)).toBe('00:00');
  expect(formatMinute(1230)).toBe('20:30');
  expect(parseTimeInput('04:30')).toBe(270);
  expect(parseTimeInput('23:59')).toBe(1439);
  expect(parseTimeInput('24:00')).toBeNull();
  expect(parseTimeInput('invalid')).toBeNull();
 });

 it('renders both local time and UTC in English and zh-TW',async()=>{
  const dailyHtml=renderToStaticMarkup(
   <BackupSchedule isOwner initialSchedule={makeSchedule({minute_utc:1230,frequency:'daily'})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(dailyHtml).toContain('Local 04:30 · 20:30 UTC');

  const weeklyHtml=renderToStaticMarkup(
   <BackupSchedule isOwner initialSchedule={makeSchedule({minute_utc:1230,frequency:'weekly',weekday:0})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(weeklyHtml).toContain('Local Monday 04:30 · Sunday 20:30 UTC');

  await i18n.changeLanguage('zh-TW');
  const zhHtml=renderToStaticMarkup(
   <BackupSchedule isOwner initialSchedule={makeSchedule({minute_utc:1230,frequency:'weekly',weekday:0})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(zhHtml).toContain('本機 星期一 04:30 · 星期日 20:30 UTC');
 });
});

describe('Full-mode warning and owner gating',()=>{
 it('shows fixed full-mode security warning for owner in en and zh-TW',async()=>{
  const html=renderToStaticMarkup(
   <BackupSchedule isOwner initialSchedule={makeSchedule({mode:'full'})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(html).toContain('data-testid="backup-full-warning"');
  expect(html).toContain('owner password hash, paired device credentials, and LAN private key');
  expect(html).not.toContain('data-testid="backup-owner-only-hint"');
  expect(html).not.toContain('<option value="full" disabled=""');

  await i18n.changeLanguage('zh-TW');
  const zh=renderToStaticMarkup(
   <BackupSchedule isOwner initialSchedule={makeSchedule({mode:'full'})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(zh).toContain('此備份含 owner 密碼雜湊、已配對裝置憑證與 LAN 私鑰，拿到的人可以冒充這台 Core。');
 });

 it('gates full mode for non-owner and disables Run now on an existing full schedule',()=>{
  // Fresh disabled Core default has mode:'full' -> non-owner draft defaults to portable, full option disabled.
  const freshNonOwner=renderToStaticMarkup(
   <BackupSchedule isOwner={false} initialSchedule={makeSchedule({enabled:false,mode:'full'})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(freshNonOwner).toContain('<option value="full" disabled=""');
  expect(freshNonOwner).toContain('data-testid="backup-owner-only-hint"');
  expect(freshNonOwner).not.toContain('data-testid="backup-full-warning"');

  // Active full schedule viewed by non-owner shows warning and disables Run now & Save.
  const activeFullNonOwner=renderToStaticMarkup(
   <BackupSchedule isOwner={false} initialSchedule={makeSchedule({enabled:true,mode:'full'})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(activeFullNonOwner).toContain('data-testid="backup-full-warning"');
  expect(activeFullNonOwner).toContain('<option value="full" disabled=""');
  expect(activeFullNonOwner).toMatch(/<button[^>]*disabled=""[^>]*>[^<]*Save schedule/);
  expect(activeFullNonOwner).toMatch(/<button[^>]*disabled=""[^>]*>.*Run now/);
 });

 it('hides local folder Browse button when connected to remote Core and shows it on local desktop',()=>{
  apiMock.isLocalDesktop.mockReturnValue(false);
  const remoteHtml=renderToStaticMarkup(
   <BackupPanel onRestored={async()=>{}} allowRestore={true} initialSchedule={makeSchedule()} initialHistory={[]}/>,
  );
  expect(remoteHtml).not.toContain('Choose folder');

  apiMock.isLocalDesktop.mockReturnValue(true);
  const localHtml=renderToStaticMarkup(
   <BackupPanel onRestored={async()=>{}} allowRestore={true} initialSchedule={makeSchedule()} initialHistory={[]}/>,
  );
  expect(localHtml).toContain('Choose folder');
 });
});

describe('Overdue banner and schedule status',()=>{
 it('detects overdue only when enabled and >2 periods have passed since last_success',()=>{
  expect(isScheduleOverdue({enabled:false,frequency:'daily',last_success:NOW-10*DAY},NOW)).toBe(false);
  expect(isScheduleOverdue({enabled:true,frequency:'daily',last_success:null},NOW)).toBe(false);
  expect(isScheduleOverdue({enabled:true,frequency:'daily',last_success:NOW-2*DAY},NOW)).toBe(false);
  expect(isScheduleOverdue({enabled:true,frequency:'daily',last_success:NOW-2*DAY-1},NOW)).toBe(true);
  expect(isScheduleOverdue({enabled:true,frequency:'weekly',last_success:NOW-14*DAY},NOW)).toBe(false);
  expect(isScheduleOverdue({enabled:true,frequency:'weekly',last_success:NOW-14*DAY-1},NOW)).toBe(true);
 });

 it('renders top overdue warning banner, running state, last error, and free space',()=>{
  const overdueHtml=renderToStaticMarkup(
   <BackupSchedule
    isOwner
    initialSchedule={makeSchedule({
     enabled:true,
     frequency:'daily',
     last_success:NOW-3*DAY,
     last_error:'Insufficient backup space',
     running:true,
     available_bytes:5*1024**3,
    })}
    timezoneOffsetMinutes={-480}
    nowSeconds={NOW}
   />,
  );
  expect(overdueHtml).toContain('data-testid="backup-overdue-banner"');
  expect(overdueHtml).toContain('data-testid="backup-running-pill"');
  expect(overdueHtml).toContain('Insufficient backup space');
  expect(overdueHtml).toContain('5.0 GiB');

  const healthyHtml=renderToStaticMarkup(
   <BackupSchedule isOwner initialSchedule={makeSchedule({enabled:true,last_success:NOW-DAY})} timezoneOffsetMinutes={-480} nowSeconds={NOW}/>,
  );
  expect(healthyHtml).not.toContain('data-testid="backup-overdue-banner"');
 });

 it('submits exact 8-field schedule payload without unknown state fields and calls Run now',async()=>{
  resetHooks(true);
  const initial=makeSchedule({enabled:false,frequency:'weekly',weekday:0,minute_utc:1230,keep:14,mode:'full',artwork:true,destination:'/backups'});
  const updated={...initial,enabled:true,minute_utc:1200,weekday:0,keep:7};
  apiMock.api.mockImplementation(async(path:string,body?:unknown)=>{
   if(path==='/backups/schedule'&&body===undefined)return initial;
   if(path==='/backups/schedule'&&body!==undefined)return updated;
   if(path==='/backups/run')return {deferred:false,path:'/backups/Galroon-backup-now'};
   return [];
  });
  const onChanged=vi.fn();
  const renderSchedule=()=>{
   hooks.cursor=0;hooks.refCursor=0;
   return nodes(BackupSchedule({isOwner:true,onChanged,initialSchedule:initial,timezoneOffsetMinutes:-480,nowSeconds:NOW}));
  };
  let tree=renderSchedule();
  // Toggle enabled on
  tree.find(el=>el.type==='input'&&el.props.role==='switch')!.props.onChange({target:{checked:true}});
  // Change local time to 04:00 (240 min) -> 20:00 UTC (1200 min) in UTC+8; weekday stays local Monday (1) -> UTC Sunday (0)
  tree=renderSchedule();
  tree.find(el=>el.type==='input'&&el.props.type==='time')!.props.onChange({target:{value:'04:00'}});
  tree=renderSchedule();
  tree.find(el=>el.type==='input'&&el.props.type==='number')!.props.onChange({target:{value:'7'}});
  tree=renderSchedule();
  const saveBtn=tree.find(el=>el.type==='button'&&textOf(el).includes('Save schedule'))!;
  expect(saveBtn.props.disabled).toBe(false);
  saveBtn.props.onClick();
  await settle();

  const postCall=apiMock.api.mock.calls.find(([p,b])=>p==='/backups/schedule'&&b!==undefined)!;
  expect(postCall[1]).toEqual({
   enabled:true,
   destination:'/backups',
   frequency:'weekly',
   minute_utc:1200,
   weekday:0,
   keep:7,
   mode:'full',
   artwork:true,
  });
  expect(Object.keys(postCall[1] as Record<string,unknown>).sort()).toEqual([
   'artwork','destination','enabled','frequency','keep','minute_utc','mode','weekday',
  ]);

  tree=renderSchedule();
  const runBtn=tree.find(el=>el.type==='button'&&textOf(el).includes('Run now'))!;
  expect(runBtn.props.disabled).toBe(false);
  runBtn.props.onClick();
  await settle();
  expect(apiMock.api).toHaveBeenCalledWith('/backups/run',{});
  expect(onChanged).toHaveBeenCalled();
 });
});

describe('Backup history badges and manual delete',()=>{
 it('renders origin/mode badges and shows Delete only for non-deleted manual backups',async()=>{
  const records:BackupRecord[]=[
   makeRecord({id:'manual-live',origin:'manual',mode:'portable',deleted:false}),
   makeRecord({id:'manual-gone',origin:'manual',mode:'full',deleted:true}),
   makeRecord({id:'sched-live',origin:'scheduled',mode:'full',deleted:false}),
  ];
  const html=renderToStaticMarkup(
   <BackupPanel onRestored={async()=>{}} allowRestore={true} initialSchedule={makeSchedule()} initialHistory={records}/>,
  );
  expect(html).toContain('Manual');
  expect(html).toContain('Scheduled');
  expect(html).toContain('Portable');
  expect(html).toContain('Full');
  expect(html).toContain('Deleted');
  // Exactly one Delete button (for manual-live only; not manual-gone and not sched-live).
  expect(html.match(/>Delete<\/button>/g)).toHaveLength(1);

  await i18n.changeLanguage('zh-TW');
  const zh=renderToStaticMarkup(
   <BackupPanel onRestored={async()=>{}} allowRestore={true} initialSchedule={makeSchedule()} initialHistory={records}/>,
  );
  expect(zh).toContain('手動');
  expect(zh).toContain('自動');
  expect(zh).toContain('可攜');
  expect(zh).toContain('完整');
  expect(zh).toContain('已刪除');
  expect(zh.match(/>刪除<\/button>/g)).toHaveLength(1);
 });

 it('deletes a manual backup via DELETE /api/backups/{id} and reloads history',async()=>{
  resetHooks(true);
  const live=makeRecord({id:'manual/1',origin:'manual',mode:'portable',deleted:false});
  const after=[{...live,deleted:true}];
  let calls=0;
  apiMock.api.mockImplementation(async(path:string)=>{
   if(path==='/backups')return calls++===0?[live]:after;
   return makeSchedule();
  });
  apiMock.deleteApi.mockResolvedValue({deleted:true});

  const renderPanel=()=>{
   hooks.cursor=0;hooks.refCursor=0;
   return nodes(BackupPanel({onRestored:async()=>{},allowRestore:true,initialHistory:[live],initialSchedule:makeSchedule()}));
  };
  let tree=renderPanel();
  const deleteBtn=tree.find(el=>el.type==='button'&&textOf(el).includes('Delete'))!;
  expect(deleteBtn).toBeDefined();
  deleteBtn.props.onClick();
  await settle();

  expect(apiMock.deleteApi).toHaveBeenCalledWith('/backups/manual%2F1');
  expect(apiMock.api).toHaveBeenCalledWith('/backups');
 });

 it('exports manual backup with selected mode and shows full-mode warning when owner picks full',async()=>{
  resetHooks(true);
  apiMock.api.mockImplementation(async(path:string,body?:unknown)=>{
   if(path==='/backups'&&body!==undefined)return {path:'/backups/Galroon-backup-manual'};
   return [];
  });
  const renderPanel=()=>{
   hooks.cursor=0;hooks.refCursor=0;
   return nodes(BackupPanel({onRestored:async()=>{},allowRestore:true,initialHistory:[],initialSchedule:makeSchedule()}));
  };
  let tree=renderPanel();
  const destInput=tree.find(el=>el.type==='input'&&el.props['aria-describedby']==='backup-destination-hint')!;
  destInput.props.onChange({target:{value:'/backups'}});
  tree=renderPanel();
  const modeSelect=tree.find(el=>el.type==='select'&&nodes(el).some(o=>o.props.value==='portable'))!;
  expect(modeSelect.props.value).toBe('portable');
  modeSelect.props.onChange({target:{value:'full'}});
  tree=renderPanel();
  // Full warning component is rendered when manual mode is full
  expect(tree.some(el=>el.type&&typeof el.type==='function'&&el.type.name==='BackupFullWarning')).toBe(true);
  const exportBtn=tree.find(el=>el.type==='button'&&textOf(el).includes('Export backup'))!;
  exportBtn.props.onClick();
  await settle();
  expect(apiMock.api).toHaveBeenCalledWith('/backups',{destination:'/backups',mode:'full'});
 });
});
