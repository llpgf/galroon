import {api,readApi,type MatchSuggestion,type SuggestionResult} from './api';
export type SuggestionBatchResult={suggestion:MatchSuggestion;result:SuggestionResult};
type Page={items:MatchSuggestion[];next:number|null};
/** Batch scope includes open suggestions on every page, even outside the current resource filter. */
export async function loadVerifiedSuggestions(signal:AbortSignal){
 const items:MatchSuggestion[]=[];let next:number|null=null;
 do{const page:Page=await readApi<Page>('/suggestions'+(next===null?'':`?before=${next}`),signal);items.push(...page.items.filter(s=>s.state==='open'&&s.verified));next=page.next;}while(next!==null&&!signal.aborted);
 return items;
}
/** Each response is retained before proceeding; an uncertain write is never retried. */
export async function confirmVerifiedSuggestions(suggestions:MatchSuggestion[],onResults:(results:SuggestionBatchResult[])=>void){
 const items=suggestions.filter(s=>s.state==='open'&&s.verified);
 for(let offset=0;offset<items.length;offset+=60){
  const chunk=items.slice(offset,offset+60);
  const response=await api<{results:SuggestionResult[]}>('/suggestions/batch-confirm',{ids:chunk.map(s=>s.id),revisions:chunk.map(s=>s.resource_revision)});
  onResults(response.results.map((result,index)=>({suggestion:chunk[index],result})));
 }
}
