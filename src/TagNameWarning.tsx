import {useTranslation} from 'react-i18next';
import {similarTagNames} from './tagNames';
export function TagNameWarning({name,tags,excludeId}:{name:string;tags:{id:string;name:string}[];excludeId?:string}){
 const {t}=useTranslation(),similar=similarTagNames(name,tags,excludeId);
 return similar.length?<p role="status">{t('tagSimilarNames',{names:similar.map(tag=>tag.name).join(', ')})}</p>:null;
}
