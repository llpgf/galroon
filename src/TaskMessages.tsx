import {useTranslation} from 'react-i18next';
import {coreText} from './coreText';

// Core activity can be a path or a phase description. Keep its message separate:
// a retained path must never mask a failure, and legacy text is not an error code.
export function TaskMessages({activity,message,state}:{activity:string;message:string;state:string}){
 const {t}=useTranslation();
 if(!activity&&!message&&state!=='failed')return null;
 return <dl className="task-messages">
  {activity&&<div><dt>{t('taskLastActivity')}</dt><dd className="path">{activity}</dd></div>}
  {(message||state==='failed')&&<div><dt>{t('taskCoreMessage')}</dt><dd className={state==='failed'?'path error-text':'path'}>{coreText(t,message)||t('taskMissingError')}</dd></div>}
 </dl>;
}
