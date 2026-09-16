import {APP_VERSION} from './version';
import {useState} from 'react';
import {invoke,isTauri} from '@tauri-apps/api/core';
import {useTranslation} from 'react-i18next';
import {api} from './api';
import {browserDiagnostics,clearBrowserDiagnostics} from './browserDiagnostics';
type Log={available:boolean;error:string|null;text:string};
export function DiagnosticsPanel({core=true}:{core?:boolean}){
 const {t}=useTranslation();const desktop=isTauri();const [busy,setBusy]=useState(false),[error,setError]=useState(''),[report,setReport]=useState<Record<string,Log>|null>(null);
 async function load(){setBusy(true);setError('');try{
  if(!desktop){setReport({browser:browserDiagnostics()});return;}
  const local=await invoke<Log>('diagnostic_logs');let server:Log={available:false,error:t('diagnosticsCoreOffline'),text:''};if(core){try{server=await api<Log>('/diagnostics');}catch(e){server.error=e instanceof Error?e.message:String(e);}}setReport({desktop:local,core:server});
 }catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}}
 function download(){if(!report)return;const url=URL.createObjectURL(new Blob([JSON.stringify({version:APP_VERSION,exported_at:new Date().toISOString(),...report},null,2)],{type:'application/json'}));const link=document.createElement('a');link.href=url;link.download='galroon-diagnostics.json';link.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
 return <section className="section"><h2>{t('diagnostics')}</h2><p>{t(desktop?'diagnosticsHint':'diagnosticsBrowserHint')}</p><button disabled={busy} onClick={()=>void load()}>{t('diagnosticsLoad')}</button>{busy&&<p role="status">{t('loading')}</p>}{error&&<p role="alert">{error}</p>}{report&&<><p>{t('diagnosticsReview')}</p>{Object.entries(report).map(([name,log])=><details key={name}><summary>{name==='core'?'Core':name==='desktop'?'Desktop':t('diagnosticsBrowser')} · {log.available?t('diagnosticsAvailable'):t('diagnosticsUnavailable')}</summary>{log.error&&<p role="alert">{log.error}</p>}<pre style={{whiteSpace:'pre-wrap',overflowWrap:'anywhere',maxHeight:280,overflowY:'auto'}}>{log.text||t('diagnosticsEmpty')}</pre></details>)}<button onClick={download}>{t('diagnosticsExport')}</button>{!desktop&&<button onClick={()=>{clearBrowserDiagnostics();setReport({browser:browserDiagnostics()});}}>{t('diagnosticsClearBrowser')}</button>}</>}</section>;
}
