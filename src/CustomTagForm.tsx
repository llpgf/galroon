import {TagNameWarning} from './TagNameWarning';
import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {api} from './api';
import {TagWorkPicker} from './TagWorkPicker';
export interface CustomTag {id:string;name:string;work_ids:string[]}
export function CustomTagForm({tags=[],titleMode,onSaved,onCancel,onBusy}:{tags?:CustomTag[];titleMode:'display'|'original';onSaved:(tag:CustomTag)=>void;onCancel:()=>void;onBusy:(busy:boolean)=>void}){
 const {t}=useTranslation();const [name,setName]=useState(''),[selected,setSelected]=useState<string[]>([]),[busy,setBusy]=useState(false),[error,setError]=useState('');
 return <form className="custom-tag-form" onSubmit={event=>{event.preventDefault();if(busy||!name.trim())return;setBusy(true);onBusy(true);setError('');void api<CustomTag>('/custom-tags',{name:name.trim(),work_ids:selected}).then(onSaved).catch(error=>setError(error.message)).finally(()=>{setBusy(false);onBusy(false);});}}>
  <label>{t('customTagName')}<input autoFocus value={name} maxLength={80} required disabled={busy} onChange={event=>setName(event.target.value)}/></label>
  <TagNameWarning name={name} tags={tags}/>
  <TagWorkPicker selected={selected} onChange={setSelected} titleMode={titleMode} disabled={busy}/>
  {error&&<p className="error-text" role="alert">{error}</p>}
  <footer><span>{t('selectedTagWorks',{count:selected.length})}</span><button type="button" disabled={busy} onClick={onCancel}>{t('cancel')}</button><button className="primary" disabled={busy||!name.trim()}>{t(busy?'savingTag':'createTag')}</button></footer>
 </form>;
}
