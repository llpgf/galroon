import {DiagnosticsPanel} from './DiagnosticsPanel';
import {useTranslation} from 'react-i18next';
import {useState} from 'react';
import {ConnectionsPanel} from './ConnectionsPanel';
import {connect,type ConnectionSnapshot} from './api';
export function ConnectionBanner({state}:{state:ConnectionSnapshot}){
 const {t}=useTranslation();const [manage,setManage]=useState(false);if(state.phase==='connected'||state.phase==='idle')return null;
 return <section className="connection-banner" aria-live="polite"><div><strong>{t('connection_'+state.phase)}</strong><p>{t(state.phase==='collection_changed'?'differentCollectionHint':state.phase==='incompatible'?'incompatibleHint':state.phase==='stopped'?'connectionStoppedHint':state.phase==='restoring'?'connectionRestoreHint':'connectionRecoveryHint')}</p>{state.error&&<p className="connection-error">{state.error}</p>}</div>{['offline','stopped','incompatible'].includes(state.phase)&&<button onClick={()=>void connect().catch(()=>{})}>{t('reconnectCore')}</button>}{state.phase==='collection_changed'&&<button onClick={()=>window.location.reload()}>{t('openDifferentCollection')}</button>}<button onClick={()=>setManage(!manage)}>{t('savedCores')}</button>{manage&&<><ConnectionsPanel/><DiagnosticsPanel core={false}/></>}</section>;
}

