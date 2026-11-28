import {useEffect,useRef,useState} from 'react';
import {useTranslation} from 'react-i18next';
import {Check,Copy,ExternalLink,ImagePlus,RefreshCw,RotateCcw,Sparkles,Upload,X} from 'lucide-react';
import {api,type Work} from './api';
import {CachedImage} from './CachedImage';
const fields=['title','original_title','description','cover','developer','released','tags','aliases'] as const;
const textFields=['title','original_title','developer','released','description','tags','aliases'] as const;
type Field=(typeof fields)[number];
type ProviderKey='vndb'|'bangumi'|'dlsite';
type PrefValue='auto'|ProviderKey|'manual';
const providerOrder:ProviderKey[]=['vndb','bangumi','dlsite'];
export type MetadataSnapshot={
 source:Record<string,unknown>;
 overrides:Record<string,unknown>;
 revision:number;
 providers?:Partial<Record<ProviderKey,Record<string,unknown>>>;
 preferences?:Partial<Record<Field,PrefValue>>;
 ids?:{vndb_id?:string|null;vndb_release?:string|null;bangumi_id?:number|null;dlsite_code?:string|null};
};
const format=(v:unknown)=>Array.isArray(v)?v.join('\n'):typeof v==='string'?v:'';
const isNonEmpty=(v:unknown)=>Array.isArray(v)?v.length>0:typeof v==='string'&&v.trim().length>0;
function providerField(snap:MetadataSnapshot,p:ProviderKey,f:Field):string{
 const v=snap.providers?.[p]?.[f];
 return isNonEmpty(v)?format(v):'';
}
function autoField(snap:MetadataSnapshot,f:Field):{value:string;from?:ProviderKey|'source'}{
 for(const p of providerOrder){
  const v=providerField(snap,p,f);
  if(v)return {value:v,from:p};
 }
 const base=format(snap.source[f]);
 return base?{value:base,from:'source'}:{value:''};
}
function effectiveField(snap:MetadataSnapshot,f:Field,pref:PrefValue,manualValue:string):string{
 if(pref==='manual')return manualValue;
 if(pref==='vndb'||pref==='bangumi'||pref==='dlsite'){
  const chosen=providerField(snap,pref,f);
  if(chosen)return chosen;
 }
 return autoField(snap,f).value;
}
function fileToBase64(file:File):Promise<string>{
 return new Promise((resolve,reject)=>{
  const reader=new FileReader();
  reader.onerror=()=>reject(new Error('Failed to read image file'));
  reader.onload=()=>{
   const result=typeof reader.result==='string'?reader.result:'';
   const idx=result.indexOf(';base64,');
   resolve(idx>=0?result.slice(idx+8):result);
  };
  reader.readAsDataURL(file);
 });
}
export function MetadataEditor({work,onClose,onSaved}:{work:Work;onClose:()=>void;onSaved:()=>Promise<void>}){
 const {t}=useTranslation();
 const [tab,setTab]=useState<'sources'|'preferences'|'edit'>('edit');
 const [snapshot,setSnapshot]=useState<MetadataSnapshot|null>(null);
 const [prefs,setPrefs]=useState<Record<Field,PrefValue>>(()=>Object.fromEntries(fields.map(f=>[f,'auto'])) as Record<Field,PrefValue>);
 const [manual,setManual]=useState<Record<Field,string>>(()=>Object.fromEntries(fields.map(f=>[f,''])) as Record<Field,string>);
 const [vndbInput,setVndbInput]=useState(work.vndb_id||'');
 const [bgmInput,setBgmInput]=useState('');
 const [dlsiteInput,setDlsiteInput]=useState('');
 const [applyEmpty,setApplyEmpty]=useState(true);
 const [fetching,setFetching]=useState<ProviderKey|null>(null);
 const [uploading,setUploading]=useState(false);
 const [notice,setNotice]=useState('');
 const [error,setError]=useState('');
 const [busy,setBusy]=useState(false);
 const fileInput=useRef<HTMLInputElement>(null);

 const hydrate=(s:MetadataSnapshot)=>{
  setSnapshot(s);
  const nextPrefs={} as Record<Field,PrefValue>;
  const nextManual={} as Record<Field,string>;
  for(const f of fields){
   const hasOverride=Object.hasOwn(s.overrides,f);
   const storedPref=s.preferences?.[f];
   nextPrefs[f]=storedPref||(hasOverride?'manual':'auto');
   nextManual[f]=hasOverride?format(s.overrides[f]):'';
  }
  setPrefs(nextPrefs);
  setManual(nextManual);
  setVndbInput(s.ids?.vndb_id||work.vndb_id||'');
  setBgmInput(s.ids?.bangumi_id?String(s.ids.bangumi_id):'');
  setDlsiteInput(s.ids?.dlsite_code||'');
 };

 useEffect(()=>{
  api<MetadataSnapshot>(`/works/${work.id}/metadata`).then(hydrate).catch(e=>setError(e instanceof Error?e.message:String(e)));
 },[work.id]);

 const fetchProvider=async(source:ProviderKey,rawId:string)=>{
  if(!snapshot||!rawId.trim())return;
  setFetching(source);setError('');setNotice('');
  try{
   const updated=await api<MetadataSnapshot>(`/works/${work.id}/metadata/fetch-source`,{
    source,id:rawId.trim(),revision:snapshot.revision,apply_empty:applyEmpty
   });
   hydrate(updated);
   await onSaved();
   setNotice(t('metadataFetchedFrom',{source:t(`metadataProvider_${source}`)}));
  }catch(e){
   setError(e instanceof Error?e.message:String(e));
  }finally{
   setFetching(null);
  }
 };

 const handleUpload=async(file:File|undefined)=>{
  if(!file)return;
  setUploading(true);setError('');setNotice('');
  try{
   const data_base64=await fileToBase64(file);
   const res=await api<{url:string}>(`/works/${work.id}/cover`,{data_base64});
   setManual(prev=>({...prev,cover:res.url}));
   setPrefs(prev=>({...prev,cover:'manual'}));
   setNotice(t('metadataCoverUploaded'));
  }catch(e){
   setError(e instanceof Error?e.message:String(e));
  }finally{
   setUploading(false);
   if(fileInput.current)fileInput.current.value='';
  }
 };

 const resetAll=()=>{
  if(!snapshot)return;
  const nextPrefs={} as Record<Field,PrefValue>;
  const nextManual={} as Record<Field,string>;
  for(const f of fields){nextPrefs[f]='auto';nextManual[f]='';}
  setPrefs(nextPrefs);setManual(nextManual);
  setNotice(t('metadataRevertedDraft'));
 };

 const save=async()=>{
  if(!snapshot)return;
  setBusy(true);setError('');
  try{
   const patch:Record<string,unknown>={};
   const preferences:Record<string,PrefValue>={};
   for(const f of fields){
    preferences[f]=prefs[f];
    if(prefs[f]==='manual'){
     patch[f]=['tags','aliases'].includes(f)?manual[f].split('\n').map(x=>x.trim()).filter(Boolean):manual[f];
    }
   }
   await api(`/works/${work.id}/metadata`,{revision:snapshot.revision,patch,reset:[],preferences});
   await onSaved();
   onClose();
  }catch(e){
   setError(e instanceof Error?e.message:String(e));
  }finally{
   setBusy(false);
  }
 };

 const previewCover=snapshot?effectiveField(snapshot,'cover',prefs.cover,manual.cover):work.cover;
 const previewTitle=snapshot?effectiveField(snapshot,'title',prefs.title,manual.title)||work.title:work.title;
 const previewDev=snapshot?effectiveField(snapshot,'developer',prefs.developer,manual.developer)||work.developer:work.developer;
 const searchName=encodeURIComponent(work.original_title||work.title);

 return <div className="scrim"><div className="metadata-modal" role="dialog" aria-modal="true" aria-label={t('editMetadata')}>
  <header className="metadata-modal-head">
   <div className="metadata-modal-summary">
    <div className="metadata-thumb">{previewCover?<CachedImage source={previewCover}/>:<span>{previewTitle.slice(0,2)}</span>}</div>
    <div>
     <h2>{t('editMetadata')}</h2>
     <p className="metadata-work-sub">{previewTitle}{previewDev?` · ${previewDev}`:''}</p>
    </div>
   </div>
   <button className="icon-button" aria-label={t('close')} disabled={busy} onClick={onClose}><X size={18}/></button>
  </header>

  <nav className="metadata-tabs" role="tablist" aria-label={t('editMetadata')}>
   {([['sources','metadataTabSources'],['preferences','metadataTabPreferences'],['edit','metadataTabEdit']] as const).map(([key,label])=>
    <button key={key} role="tab" aria-selected={tab===key} className={tab===key?'active':''} onClick={()=>setTab(key)}>{t(label)}</button>
   )}
  </nav>

  <div className="metadata-modal-body">
   {notice&&<div className="metadata-notice" role="status"><Check size={15}/><span>{notice}</span></div>}
   {error&&<p className="error-text" role="alert">{error}</p>}
   {!snapshot?<p className="muted">{t('loading')}</p>:<>
    {tab==='sources'&&<div className="metadata-sources-tab">
     <p className="muted">{t('metadataSourcesHint')}</p>
     <label className="checkbox-label"><input type="checkbox" checked={applyEmpty} onChange={e=>setApplyEmpty(e.target.checked)}/><span>{t('metadataApplyEmpty')}</span></label>

     <section className="metadata-source-card">
      <div className="metadata-source-head">
       <div><h3>VNDB</h3><p className="muted">{t('metadataVndbHint')}</p></div>
       <a className="text-button" href={`https://vndb.org/v?sq=${searchName}`} target="_blank" rel="noreferrer">{t('metadataSearchExternal')} <ExternalLink size={13}/></a>
      </div>
      <div className="metadata-source-row">
       <input value={vndbInput} placeholder="v12345" onChange={e=>setVndbInput(e.target.value)}/>
       <button disabled={fetching!==null||!vndbInput.trim()} onClick={()=>void fetchProvider('vndb',vndbInput)}>
        {fetching==='vndb'&&<RefreshCw size={14} className="spin"/>}{t('metadataFetchSource')}
       </button>
      </div>
      {snapshot.providers?.vndb&&<small className="metadata-source-ok"><Check size={13}/>{t('metadataSourceLoaded',{title:format(snapshot.providers.vndb.title)||t('unknown')})}</small>}
     </section>

     <section className="metadata-source-card">
      <div className="metadata-source-head">
       <div><h3>Bangumi</h3><p className="muted">{t('metadataBangumiHint')}</p></div>
       <a className="text-button" href={`https://bgm.tv/subject_search/${searchName}?cat=4`} target="_blank" rel="noreferrer">{t('metadataSearchExternal')} <ExternalLink size={13}/></a>
      </div>
      <div className="metadata-source-row">
       <input value={bgmInput} placeholder="https://bgm.tv/subject/123456" onChange={e=>setBgmInput(e.target.value)}/>
       <button disabled={fetching!==null||!bgmInput.trim()} onClick={()=>void fetchProvider('bangumi',bgmInput)}>
        {fetching==='bangumi'&&<RefreshCw size={14} className="spin"/>}{t('metadataFetchSource')}
       </button>
      </div>
      {snapshot.providers?.bangumi&&<small className="metadata-source-ok"><Check size={13}/>{t('metadataSourceLoaded',{title:format(snapshot.providers.bangumi.title)||t('unknown')})}</small>}
     </section>

     <section className="metadata-source-card">
      <div className="metadata-source-head">
       <div><h3>DLsite</h3><p className="muted">{t('metadataDlsiteHint')}</p></div>
       <a className="text-button" href={`https://www.dlsite.com/maniax/fsr/=/keyword/${searchName}`} target="_blank" rel="noreferrer">{t('metadataSearchExternal')} <ExternalLink size={13}/></a>
      </div>
      <div className="metadata-source-row">
       <input value={dlsiteInput} placeholder="RJ351325 / VJ01001192" onChange={e=>setDlsiteInput(e.target.value)}/>
       <button disabled={fetching!==null||!dlsiteInput.trim()} onClick={()=>void fetchProvider('dlsite',dlsiteInput)}>
        {fetching==='dlsite'&&<RefreshCw size={14} className="spin"/>}{t('metadataFetchSource')}
       </button>
      </div>
      {snapshot.providers?.dlsite&&<small className="metadata-source-ok"><Check size={13}/>{t('metadataSourceLoaded',{title:format(snapshot.providers.dlsite.title)||t('unknown')})}</small>}
     </section>
    </div>}

    {tab==='preferences'&&<div className="metadata-prefs-tab">
     <p className="muted">{t('metadataPrefsHint')}</p>
     <div className="metadata-prefs-table">
      {fields.map(f=>{
       const choices:PrefValue[]=['auto',...providerOrder,'manual'];
       return <div key={f} className="metadata-pref-row">
        <span className="metadata-pref-label">{t(`field_${f}`)}</span>
        <div className="metadata-pref-radios" role="radiogroup" aria-label={t(`field_${f}`)}>
         {choices.map(choice=>{
          const hasData=choice==='auto'||choice==='manual'||!!providerField(snapshot,choice,f);
          return <label key={choice} className={`metadata-radio-pill ${prefs[f]===choice?'selected':''} ${!hasData?'empty':''}`}>
           <input type="radio" name={`pref-${f}`} checked={prefs[f]===choice} onChange={()=>setPrefs(prev=>({...prev,[f]:choice}))}/>
           <span>{t(`metadataPref_${choice}`)}</span>
          </label>;
         })}
        </div>
       </div>;
      })}
     </div>
    </div>}

    {tab==='edit'&&<div className="metadata-edit-tab">
     <section className="metadata-cover-section">
      <h3>{t('field_cover')}</h3>
      <div className="metadata-cover-grid">
       <div className="metadata-cover-sources">
        {(['auto',...providerOrder] as const).map(choice=>{
         const url=choice==='auto'?autoField(snapshot,'cover').value:providerField(snapshot,choice,'cover');
         return <label key={choice} className={`metadata-cover-option ${prefs.cover===choice?'selected':''}`}>
          <input type="radio" name="cover-choice" checked={prefs.cover===choice} onChange={()=>setPrefs(prev=>({...prev,cover:choice}))}/>
          <div className="metadata-cover-option-body">
           <strong>{choice==='auto'&&<Sparkles size={13}/>}{t(`metadataPref_${choice}`)}</strong>
           {url?<div className="metadata-cover-thumb"><CachedImage source={url}/></div>:<small className="muted">{t('metadataNoImage')}</small>}
          </div>
         </label>;
        })}
       </div>
       <div className={`metadata-cover-manual ${prefs.cover==='manual'?'selected':''}`}>
        <label className="metadata-radio-head">
         <input type="radio" name="cover-choice" checked={prefs.cover==='manual'} onChange={()=>setPrefs(prev=>({...prev,cover:'manual'}))}/>
         <strong>{t('metadataPref_manual')}</strong>
        </label>
        <div className="metadata-cover-thumb large">
         {manual.cover?<CachedImage source={manual.cover}/>:<span className="muted"><ImagePlus size={24}/>{t('metadataNoImage')}</span>}
        </div>
        <input ref={fileInput} type="file" accept="image/png,image/jpeg,image/webp" hidden onChange={e=>void handleUpload(e.target.files?.[0])}/>
        <button type="button" disabled={uploading} onClick={()=>fileInput.current?.click()}>
         {uploading?<RefreshCw size={14} className="spin"/>:<Upload size={14}/>}{t(uploading?'loading':'metadataUploadCover')}
        </button>
        <input type="text" value={manual.cover} placeholder={t('metadataCoverUrlPlaceholder')} onChange={e=>{setManual(prev=>({...prev,cover:e.target.value}));setPrefs(prev=>({...prev,cover:'manual'}));}}/>
       </div>
      </div>
     </section>

     {textFields.map(f=>{
      const auto=autoField(snapshot,f);
      const activePref=prefs[f];
      const chosenMissing=(activePref==='vndb'||activePref==='bangumi'||activePref==='dlsite')&&!providerField(snapshot,activePref,f);
      const copyable=auto.value||format(work[f as keyof Work]);
      return <section key={f} className="metadata-field-card">
       <div className="metadata-field-head"><h3>{t(`field_${f}`)}</h3></div>
       <div className="metadata-field-split">
        <div className="metadata-field-sources">
         <label className={`metadata-source-radio ${activePref==='auto'?'selected':''}`}>
          <input type="radio" name={`edit-${f}`} checked={activePref==='auto'} onChange={()=>setPrefs(prev=>({...prev,[f]:'auto'}))}/>
          <div><strong>{t('metadataPref_auto')}</strong><p>{auto.value||t('unknown')}</p></div>
         </label>
         {providerOrder.map(p=>{
          const val=providerField(snapshot,p,f);
          if(!val&&activePref!==p)return null;
          return <label key={p} className={`metadata-source-radio ${activePref===p?'selected':''}`}>
           <input type="radio" name={`edit-${f}`} checked={activePref===p} onChange={()=>setPrefs(prev=>({...prev,[f]:p}))}/>
           <div><strong>{t(`metadataProvider_${p}`)}</strong><p>{val||t('unknown')}</p></div>
          </label>;
         })}
         {chosenMissing&&<small className="muted">{t('metadataFallbackUsed')}</small>}
        </div>
        <div className={`metadata-field-manual ${activePref==='manual'?'selected':''}`}>
         <div className="metadata-manual-top">
          <label className="metadata-radio-head">
           <input type="radio" name={`edit-${f}`} checked={activePref==='manual'} onChange={()=>setPrefs(prev=>({...prev,[f]:'manual'}))}/>
           <strong>{t('metadataPref_manual')}</strong>
          </label>
          {copyable&&<button type="button" className="icon-button" title={t('metadataCopyFromSource')} aria-label={t('metadataCopyFromSource')} onClick={()=>{setManual(prev=>({...prev,[f]:copyable}));setPrefs(prev=>({...prev,[f]:'manual'}));}}><Copy size={14}/></button>}
         </div>
         <textarea rows={f==='description'?4:2} value={manual[f]} placeholder={copyable||t('unknown')} onChange={e=>{setManual(prev=>({...prev,[f]:e.target.value}));setPrefs(prev=>({...prev,[f]:'manual'}));}}/>
        </div>
       </div>
      </section>;
     })}
    </div>}
   </>}
  </div>

  <footer className="metadata-modal-foot">
   <button type="button" disabled={busy||!snapshot} onClick={resetAll}><RotateCcw size={14}/>{t('metadataRevertAll')}</button>
   <div className="inline">
    <button type="button" disabled={busy} onClick={onClose}>{t('cancel')}</button>
    <button type="button" className="primary" disabled={busy||!snapshot} onClick={()=>void save()}>{t('save')}</button>
   </div>
  </footer>
 </div></div>;
}
