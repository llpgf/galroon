import {useEffect,useMemo,useState} from 'react';
import {readApi} from './api';
import {memberPagePath,type MemberPage} from './resourceMemberSelection';
export function useResourceMembers(initial:MemberPage,query:string){
 const [trail,setTrail]=useState<{query:string;cursors:(string|null)[]}>({query:'',cursors:[null]});
 useEffect(()=>setTrail({query,cursors:[null]}),[query]);
 const cursors=useMemo(()=>trail.query===query?trail.cursors:[null],[trail,query]),before=cursors.at(-1)||null;
 const [state,setState]=useState<{query:string;before:string|null;data:MemberPage|null;error:string}>({query:'',before:null,data:initial,error:''});
 useEffect(()=>{const c=new AbortController();const timer=setTimeout(()=>{void readApi<MemberPage>(memberPagePath(initial.source.id,query,initial,before),c.signal).then(data=>{if(!c.signal.aborted)setState({query,before,data,error:''});}).catch(e=>{if(!c.signal.aborted)setState({query,before,data:null,error:e instanceof Error?e.message:String(e)});});},200);return()=>{clearTimeout(timer);c.abort();};},[initial,query,before]);
 const current=state.query===query&&state.before===before,data=current?state.data:null;
 return {data,error:current?state.error:'',loading:!current,next:()=>{if(data?.next)setTrail({query,cursors:[...cursors,data.next]});},previous:()=>{if(cursors.length>1)setTrail({query,cursors:cursors.slice(0,-1)});},pageIndex:cursors.length-1};
}
