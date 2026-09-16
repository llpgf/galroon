import {useEffect,useState} from 'react';
import {readApi} from './api';
export type TagMembershipBase={id:string;revision:number;impact_revision?:number};
export function useTagMembership(tag:TagMembershipBase|undefined,ids:string[]){
 const key=JSON.stringify([tag?.id,tag?.revision,tag?.impact_revision,[...new Set(ids)].sort()]);
 const [state,setState]=useState<{key:string;members:string[];error:string}|null>(null);
 useEffect(()=>{const controller=new AbortController();
 void (async()=>{try{const requested=JSON.parse(key)[3] as string[],members:string[]=[];if(tag){if(tag.impact_revision==null)throw Error('Tag membership is unavailable. Refresh tags.');for(let start=0;start<requested.length;start+=60){const chunk=requested.slice(start,start+60),params:URLSearchParams=new URLSearchParams({ids:JSON.stringify(chunk),revision:String(tag.revision),impact_revision:String(tag.impact_revision)});const result:{ids:string[];members:string[];revision:number;impact_revision:number}=await readApi<{ids:string[];members:string[];revision:number;impact_revision:number}>(`/custom-tags/${encodeURIComponent(tag.id)}/membership?${params}`,controller.signal);if(result.revision!==tag.revision||result.impact_revision!==tag.impact_revision||JSON.stringify(result.ids)!==JSON.stringify(chunk)||new Set(result.members).size!==result.members.length||result.members.some(id=>!chunk.includes(id)))throw Error('Tag membership changed. Refresh tags.');members.push(...result.members);}}if(!controller.signal.aborted)setState({key,members,error:''});}catch(e){if(!controller.signal.aborted)setState({key,members:[],error:e instanceof Error?e.message:String(e)});}})();return()=>controller.abort();},[key]);
 const ready=state?.key===key;return {members:ready?state.members:[],error:ready?state.error:'',loading:!ready};
}
