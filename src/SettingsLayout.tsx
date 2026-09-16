import {useState,type ReactNode} from 'react';
import {useTranslation} from 'react-i18next';
import {Palette,LibraryBig,Users,Archive} from 'lucide-react';

export function SettingsLayout({appearance,collection,account,backup}:{appearance:ReactNode;collection:ReactNode;account:ReactNode;backup:ReactNode}){
 const {t}=useTranslation();
 const [selected,setSelected]=useState('appearance');
 const sections=[
  {id:'appearance',icon:Palette,content:appearance},
  {id:'collection',icon:LibraryBig,content:collection},
  {id:'account',icon:Users,content:account},
  {id:'backup',icon:Archive,content:backup},
 ].filter(section=>section.content);
 const active=sections.some(section=>section.id===selected)?selected:sections[0]?.id;
 return <div className="settings-layout">
  <div className="settings-categories" role="tablist" aria-label={t('settingsCategories')}>
   {sections.map(({id,icon:Icon},index)=><button key={id} id={'settings-tab-'+id} role="tab" aria-selected={active===id} aria-controls={'settings-panel-'+id} tabIndex={active===id?0:-1} onClick={()=>setSelected(id)} onKeyDown={event=>{
    let next=index;
    if(event.key==='ArrowDown'||event.key==='ArrowRight')next=(index+1)%sections.length;
    else if(event.key==='ArrowUp'||event.key==='ArrowLeft')next=(index+sections.length-1)%sections.length;
    else if(event.key==='Home')next=0;
    else if(event.key==='End')next=sections.length-1;
    else return;
    event.preventDefault();setSelected(sections[next].id);document.getElementById('settings-tab-'+sections[next].id)?.focus();
   }}><Icon size={18}/><span>{t('settings_'+id)}</span></button>)}
  </div>
  <div className="settings-panels">{sections.map(({id,content})=><section key={id} id={'settings-panel-'+id} role="tabpanel" aria-labelledby={'settings-tab-'+id} hidden={active!==id} tabIndex={0}>
   <header className="settings-section-heading"><h2>{t('settings_'+id)}</h2><p>{t('settings_'+id+'_hint')}</p></header>{content}
  </section>)}</div>
 </div>;
}
