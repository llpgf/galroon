import {useState} from 'react';
import {useTranslation} from 'react-i18next';
import {AddToList,type ListMember} from './AddToList';
export function AddToListButton({member}:{member:ListMember}){const {t}=useTranslation();const [scope,setScope]=useState<ListMember[]|null>(null);return <><button onClick={()=>setScope([{...member}])}>{t('listAdd')}</button>{scope&&<AddToList members={scope} onClose={()=>setScope(null)}/>}</>;}
