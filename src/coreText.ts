import type {TFunction} from 'i18next';
// Core reports review reasons and task outcomes as fixed English sentences. Known ones are
// shown in the interface language; anything else is shown as Core wrote it.
const exact:Record<string,string>={
 'Search incomplete. Refresh when VNDB is available.':'coreText_searchIncomplete',
 'No usable work title in this resource.':'coreText_noTitle',
 'No VNDB candidate found.':'coreText_noCandidate',
 'Multiple works have equally strong evidence.':'coreText_ambiguous',
 'Only a partial title matches; the subtitle or edition needs confirmation.':'coreText_partialTitle',
 'The title and candidate have different sequel numbers.':'coreText_numberConflict',
 'The name is too short or the candidate evidence is not exact.':'coreText_weakEvidence',
 'Source title evidence is missing.':'coreText_missingTitles',
 'Source title cannot be interpreted.':'coreText_unreadableTitle',
 'Scan complete':'coreText_scanComplete',
 'Scan completed with errors. Some folders could not be read.':'coreText_scanErrors',
 'Matching complete. Uncertain resources remain Unmatched.':'coreText_matchComplete',
};
const prefixed:[string,string][]=[['A different source name needs review: ','coreText_otherSourceName']];
export function coreText(t:TFunction,message:string|null|undefined):string{
 if(!message)return '';
 const key=exact[message];if(key)return t(key,{defaultValue:message});
 for(const [prefix,k] of prefixed)if(message.startsWith(prefix))return t(k,{name:message.slice(prefix.length),defaultValue:message});
 return message;
}
