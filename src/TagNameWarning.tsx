import {useTranslation} from 'react-i18next';
import {similarTagNames,similarProviderTags} from './tagNames';
export function TagNameWarning({name,tags,excludeId,vndbTags=[]}:{name:string;tags:{id:string;name:string}[];excludeId?:string;vndbTags?:string[]}){
 const {t}=useTranslation(),similar=similarTagNames(name,tags,excludeId),provider=similarProviderTags(name,vndbTags);
 return similar.length||provider.length?<>{similar.length>0&&<p role="status">{t('tagSimilarNames',{names:similar.map(tag=>tag.name).join(', ')})}</p>}{provider.length>0&&<p role="status">{t('tagSimilarVndb',{names:provider.join(', ')})}</p>}</>:null;
}
