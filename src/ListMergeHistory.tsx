import {useTranslation} from 'react-i18next';
export type ListVariant={title:string;notes:string;preferred_release_id?:string|null;added?:number};
export function ListMergeHistory({variants=[],editionName,busy=false,onNotes,onEdition}:{variants?:ListVariant[];editionName:(id:string)=>string;busy?:boolean;onNotes?:(s:string)=>void;onEdition?:(s:string)=>void}){
 const {t}=useTranslation();if(!variants.length)return null;
 return <details className="list-merge-history"><summary>{t('listMergeHistory',{count:variants.length})}</summary>{variants.filter(Boolean).map((v,i)=><article key={i}><h3>{v.title}</h3><p style={{whiteSpace:'pre-wrap'}}>{v.notes||t('listNoNotes')}</p><small>{t('listEntryEdition')}: {v.preferred_release_id?editionName(v.preferred_release_id):t('listNoOverride')}</small>{onNotes&&onEdition&&<div className="inline"><button type="button" disabled={busy} onClick={()=>onNotes(v.notes)}>{t('listUseNotes')}</button><button type="button" disabled={busy} onClick={()=>onEdition(v.preferred_release_id||'')}>{t('listUseEdition')}</button></div>}</article>)}</details>;
}
