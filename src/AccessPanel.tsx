import {useEffect,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {ShieldCheck,Copy,Check} from 'lucide-react';
import {api,coreUrl} from './api';
type Device={id:string;name:string;role:string;created:number;expires:number|null;last_used:number;revoked:boolean};
type PairingCode={id:string;name:string;role:'desktop'|'web';expires:number};
type PairingCreated={id?:string;role:'desktop'|'web';code:string;expires:number;name:string;pairing_string?:string;certificate_sha256?:string};
type NetworkSettings={version:number;enabled:boolean;address:string;port:number;hostnames:string[]};
type NetworkInfo={settings:NetworkSettings|null;status:string;spki_fingerprint?:string;certificate_sha256?:string};
type NetworkSaved={settings:NetworkSettings;restart_required:boolean;message?:string};
export function AccessPanel(){
 const {t}=useTranslation();
 const [configured,setConfigured]=useState<boolean|null>(null);
 const [password,setPassword]=useState('');
 const [confirm,setConfirm]=useState('');
 const [review,setReview]=useState(false);
 const [deviceName,setDeviceName]=useState('');
 const [pairRole,setPairRole]=useState<'desktop'|'web'>('desktop');
 const [devices,setDevices]=useState<Device[]>([]);
 const [codes,setCodes]=useState<PairingCode[]>([]);
 const [pair,setPair]=useState<PairingCreated|null>(null);
 const [copied,setCopied]=useState(false);
 const [network,setNetwork]=useState<NetworkInfo|null>(null);
 const [lanEnabled,setLanEnabled]=useState(false);
 const [lanAddress,setLanAddress]=useState('');
 const [lanPort,setLanPort]=useState('14830');
 const [lanHostnames,setLanHostnames]=useState('');
 const [restartRequired,setRestartRequired]=useState(false);
 const [restartNotice,setRestartNotice]=useState('');
 const [now,setNow]=useState(Date.now()/1000);
 const [busy,setBusy]=useState(false);
 const [error,setError]=useState('');
 const [message,setMessage]=useState('');
 const reload=async(syncNetwork=false)=>{
  const [status,list,pending,net]=await Promise.all([
   api<{configured:boolean}>('/access/status'),
   api<Device[]>('/access/devices'),
   api<PairingCode[]>('/access/pairing-codes'),
   api<NetworkInfo>('/access/network'),
  ]);
  setConfigured(status.configured);
  setDevices(list);
  setCodes(pending);
  setNetwork(net);
  if(syncNetwork){
   if(net.settings){
    setLanEnabled(net.settings.enabled);
    setLanAddress(net.settings.address);
    setLanPort(String(net.settings.port));
    setLanHostnames(net.settings.hostnames.join(', '));
   }else{
    setLanEnabled(false);
    setLanAddress('');
    setLanPort('14830');
    setLanHostnames('');
   }
  }
 };
 useEffect(()=>{
  void reload(true).catch(e=>setError(e instanceof Error?e.message:String(e)));
  const timer=setInterval(()=>setNow(Date.now()/1000),1000);
  return()=>clearInterval(timer);
 },[]);
 const act=async(fn:()=>Promise<void>)=>{
  setBusy(true);setError('');setMessage('');
  try{await fn();}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setBusy(false);}
 };
 const copyPairingString=async(value:string)=>{
  try{
   if(!navigator.clipboard?.writeText)throw new Error('Clipboard unavailable');
   await navigator.clipboard.writeText(value);
   setCopied(true);
  }catch{
   setError(t('copyFailed'));
  }
 };
 const isExpired=(d:Device)=>d.expires!==null&&d.expires>0&&d.expires<=now;
 const portNumber=Number(lanPort);
 const validPort=lanPort.trim()!==''&&Number.isInteger(portNumber)&&portNumber>=1&&portNumber<=65535;
 const activeCodes=codes.filter(c=>c.expires>now);
 return <section className="section access-panel">
  <h2><ShieldCheck size={20}/> {t('accessTitle')}</h2>
  <p>{t('accessHint')}</p>
  {configured===null?<p>{t('loading')}</p>:<>
   <h3>{t(configured?'changeOwnerPassword':'setupOwner')}</h3>
   <p>{t('ownerPasswordHint')}</p>
   <div className="access-fields">
    <label>{t('ownerPassword')}<input type="password" autoComplete="new-password" value={password} onChange={e=>{setPassword(e.target.value);setReview(false);}}/></label>
    <label>{t('confirmOwnerPassword')}<input type="password" autoComplete="new-password" value={confirm} onChange={e=>{setConfirm(e.target.value);setReview(false);}}/></label>
   </div>
   {review?<div className="plan-item">
    <p>{t(configured?'passwordResetReview':'ownerSetupReview')}</p>
    <button className="primary" disabled={busy} onClick={()=>void act(async()=>{await api(configured?'/access/password':'/access/setup',{password});setPassword('');setConfirm('');setReview(false);setPair(null);await reload();setMessage(t('ownerSaved'));})}>{t(configured?'confirmPasswordReset':'confirmOwnerSetup')}</button>
   </div>:<button disabled={busy||Array.from(password).length<12||password!==confirm} onClick={()=>setReview(true)}>{t('reviewOwnerSettings')}</button>}

   <h3>{t('lanAccessTitle')}</h3>
   <p>{t('lanAccessHint')}</p>
   <form aria-label={t('lanAccessTitle')} onSubmit={e=>{
    e.preventDefault();
    void act(async()=>{
     const hostnames=lanHostnames.split(/[,\n]/).map(s=>s.trim()).filter(Boolean);
     const saved=await api<NetworkSaved>('/access/network',{enabled:lanEnabled,address:lanAddress.trim(),port:portNumber,hostnames});
     setRestartRequired(saved.restart_required!==false);
     setRestartNotice(saved.message||t('lanRestartRequired'));
     await reload(true);
     setMessage(t('lanSettingsSaved'));
    });
   }}>
    <label className="checkbox-label"><input type="checkbox" role="switch" checked={lanEnabled} disabled={busy} onChange={e=>setLanEnabled(e.target.checked)}/>{t('lanEnabled')}</label>
    <div className="access-fields">
     <label>{t('lanAddress')}<input value={lanAddress} disabled={busy} placeholder="192.168.1.10" spellCheck={false} onChange={e=>setLanAddress(e.target.value)}/></label>
     <label>{t('lanPort')}<input type="number" min={1} max={65535} value={lanPort} disabled={busy} onChange={e=>setLanPort(e.target.value)}/></label>
    </div>
    <label>{t('lanHostnames')}<input value={lanHostnames} disabled={busy} placeholder="nas.local" spellCheck={false} onChange={e=>setLanHostnames(e.target.value)}/><small>{t('lanHostnamesHint')}</small></label>
    <button className="primary" disabled={busy||!lanAddress.trim()||!validPort}>{t('saveLanSettings')}</button>
   </form>
   {restartRequired&&<div className="plan-item" role="status"><strong>{t('lanRestartRequiredTitle')}</strong><p>{restartNotice||t('lanRestartRequired')}</p></div>}
   {(network?.certificate_sha256||network?.spki_fingerprint)&&<div className="plan-item">
    <strong>{t('lanCertificateTitle')}</strong>
    {network.certificate_sha256&&<p><span>{t('lanCertificateSha256')}: </span><span className="path">{network.certificate_sha256}</span></p>}
    {network.spki_fingerprint&&<p><span>{t('lanSpkiFingerprint')}: </span><span className="path">{network.spki_fingerprint}</span></p>}
   </div>}

   {configured&&<>
    <h3>{t('pairDevice')}</h3>
    <p>{t('pairHint')}</p>
    <div className="access-fields">
     <label>{t('deviceName')}<input value={deviceName} maxLength={80} onChange={e=>setDeviceName(e.target.value)}/></label>
     <label>{t('pairRole')}<select value={pairRole} disabled={busy} onChange={e=>setPairRole(e.target.value as 'desktop'|'web')}><option value="desktop">{t('pairRoleDesktop')}</option><option value="web">{t('pairRoleWeb')}</option></select></label>
    </div>
    <button disabled={busy||!deviceName.trim()} onClick={()=>void act(async()=>{
     const created=await api<PairingCreated>('/access/pairing',{name:deviceName.trim(),role:pairRole});
     setPair(created);setCopied(false);await reload();
    })}>{t('createPairCode')}</button>
    {pair&&<div className="pair-code" role="status">
     {pair.expires>now?<>
      <strong>{pair.code}</strong>
      <span>{t(pair.role==='web'?'readOnlyBrowser':'pairedDesktop')} · {t('pairExpires',{seconds:Math.max(0,Math.ceil(pair.expires-now))})}</span>
      {pair.pairing_string&&<div className="plan-item">
       <span>{t('lanPairingString')}</span>
       <p className="path">{pair.pairing_string}</p>
       <button type="button" onClick={()=>void copyPairingString(pair.pairing_string!)}>{copied?<><Check size={15}/>{t('copied')}</>:<><Copy size={15}/>{t('copyPairingString')}</>}</button>
       {pair.certificate_sha256&&<small>{t('lanCertificateSha256')}: {pair.certificate_sha256}</small>}
      </div>}
     </>:t('pairExpired')}
    </div>}

    <h3>{t('pendingPairingCodes')}</h3>
    {activeCodes.length===0&&<p className="muted">{t('noPendingPairingCodes')}</p>}
    {activeCodes.map(c=><div className="access-device" key={c.id}>
     <div>
      <strong>{c.name}</strong>
      <p>{t(c.role==='web'?'readOnlyBrowser':'pairedDesktop')} · {t('pairExpires',{seconds:Math.max(0,Math.ceil(c.expires-now))})}</p>
     </div>
     <button disabled={busy} onClick={()=>void act(async()=>{
      await api(`/access/pairing-codes/${encodeURIComponent(c.id)}/revoke`,{});
      if(pair?.id===c.id)setPair(null);
      await reload();
      setMessage(t('pairingCodeRevoked'));
     })}>{t('revokePairingCode')}</button>
    </div>)}

    <h3>{t('webAccess')}</h3>
    <p>{t('webAccessHint')}</p>
    <p className="path">{coreUrl()}/</p>
    <div className="inline">
     <h3>{t('signedInDevices')}</h3>
     <button disabled={busy} onClick={()=>void act(()=>reload())}>{t('refreshDevices')}</button>
    </div>
    {devices.length===0&&<p>{t('noDevices')}</p>}
    {devices.map(d=><div className="access-device" key={d.id}>
     <div>
      <strong>{d.name}</strong>
      <p>{t(d.role==='web'?'readOnlyBrowser':d.role==='web_owner'?'ownerBrowser':'pairedDesktop')} · {t(d.revoked?'revoked':isExpired(d)?'expired':'activeDevice')}</p>
      <small>{t('lastUsed')}: {new Date(d.last_used*1000).toLocaleString()}</small>
     </div>
     {!d.revoked&&!isExpired(d)&&<button disabled={busy} onClick={()=>void act(async()=>{await api(`/access/devices/${d.id}/revoke`,{});await reload();setMessage(t('deviceRevoked'));})}>{t('revokeDevice')}</button>}
    </div>)}
   </>}
  </>}
  {message&&<p role="status">{message}</p>}
  {error&&<p className="error-text" role="alert">{error}</p>}
 </section>;
}
