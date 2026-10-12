import type {TFunction} from 'i18next';
// Core reports review reasons and task outcomes as fixed English sentences. Known ones are
// shown in the interface language; anything else is shown as Core wrote it.
const exact:Record<string,string>={
 'Search incomplete. Refresh when VNDB is available.':'coreText_searchIncomplete',
 'VNDB has too many works with this name to be sure. Choose the work manually.':'coreText_tooManyResults',
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
 'Not checked against VNDB yet.':'coreText_notChecked',
 'Local device identity is invalid':'coreText_deviceIdentity',
 "The folder's brand and release date belong to a different work with the same name.":'coreText_releaseConflict',
 'VNDB could not be reached within 8 seconds. Try again later.':'coreText_vndbUnreachable',
 'Collection or filters changed. Reload before continuing.':'coreText_collectionChanged',
 'Collection or filters changed. Reload before selecting works.':'coreText_collectionChanged',
};
const prefixed:[string,string][]=[['A different source name needs review: ','coreText_otherSourceName'],['File name is not valid UTF-8 and was skipped: ','coreText_nonUtf8Skipped'],['VNDB returned ','coreText_vndbStatus']];
export function coreText(t:TFunction,message:string|null|undefined):string{
 if(!message)return '';
 if(revisionConflicts.has(message))return t('coreText_revisionConflict');
 const key=exact[message];if(key)return t(key,{defaultValue:message});
 for(const [prefix,k] of prefixed)if(message.startsWith(prefix))return t(k,{name:message.slice(prefix.length),defaultValue:message});
 return message;
}

const revisionConflicts=new Set(['List changed. Reload first.','A work changed. Reload the grouping preview.','Tag impact changed. Reload and review deletion again.','Tag or collection changed. Refresh tags before continuing.','Collection changed. Close and reopen the edition editor before saving.','Edition changed. Close and reopen the edition editor before saving.','Edition membership changed. Close and reopen the editor.','This work changed. Reload before editing.', 'The work changed. Reload the grouping preview.', 'Resource bindings changed. Reload the grouping preview.', 'A selected work changed. Reload first.', 'A selected work changed. Reload the collection and try again.', 'Tag changed. Reload before editing.', 'Tag changed or deleted. Reload before editing.', 'Relationship corrections changed. Reload before editing.', 'Watch settings changed. Reload before saving.', 'Destination resource changed. Reload the preview.', 'File membership changed. Reload the preview.', 'Source resource changed. Reload its files.', 'Private classifications changed. Reload the split preview.', 'Smart list changed. Reload before editing.', 'Resource changed. Reload its references before saving.', 'Work changed. Reload before selecting a preferred edition.', 'Edition changed. Reload before saving.', 'Smart list changed. Reload before computing.', 'Relationship decisions changed. Reload first.', 'Entity changed. Reload before editing.', 'Collection name changed. Refresh before saving.', 'Issue changed. Refresh before retrying.', 'Issue changed. Refresh before trying again.', 'Issue changed. Refresh before reviewing the source.', 'List changed. Reload from the first page.']);
export function revisionConflictText(t:TFunction,message:string):string{return revisionConflicts.has(message)?t('coreText_revisionConflict'):message;}
