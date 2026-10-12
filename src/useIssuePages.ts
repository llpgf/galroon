import {useEffect,useRef,useState} from 'react';
import {readApi,type MatchSuggestion} from './api';
export type Issue={suggestion?:MatchSuggestion|null;id:number;resource_id:string|null;kind:string;category?:string;state:string;reason:string;revision:number;retry_pending?:boolean;title:string|null;path:string|null;source:string|null};
type Page={items:Issue[];next:number|null};

/** Re-read the loaded window using fresh continuations, without dropping its rows between requests. */
export async function readIssueWindow(read:(cursor:number|null)=>Promise<Page>,count:number,signal:AbortSignal){
 const pages:Page[]=[];let cursor:number|null=null;
 do{
  const page=await read(cursor);
  if(signal.aborted)throw new DOMException('Request cancelled','AbortError');
  pages.push(page);cursor=page.next;
 }while(cursor!==null&&pages.length<count);
 return pages;
}

export function useIssuePages(filter:string,generation:number){
 const saved=useRef<{filter:string;pages:Page[]}>({filter,pages:[]}),request=useRef<AbortController|null>(null);
 const [state,setState]=useState<{filter:string;pages:Page[];busy:boolean;error:string}>({filter,pages:[],busy:true,error:''});
 async function load(cursor:number|null=null){
  request.current?.abort();const controller=new AbortController();request.current=controller;
  const pages=saved.current.filter===filter?saved.current.pages:[];
  setState({filter,pages,busy:true,error:''});
  const read=(before:number|null)=>readApi<Page>(`/issues?state=${filter}${before===null?'':'&before='+before}`,controller.signal);
  try{
   const fresh=cursor===null?await readIssueWindow(read,Math.max(1,pages.length),controller.signal):[...pages,await read(cursor)];
   if(controller.signal.aborted)return;
   saved.current={filter,pages:fresh};setState({filter,pages:fresh,busy:false,error:''});
  }catch(error){if(!controller.signal.aborted)setState({filter,pages,busy:false,error:error instanceof Error?error.message:String(error)});}
 }
 useEffect(()=>{if(saved.current.filter!==filter)saved.current={filter,pages:[]};void load();return()=>request.current?.abort();},[filter,generation]);
 const current=state.filter===filter,pages=current?state.pages:[],next=pages.at(-1)?.next??null;
 return {items:pages.flatMap(page=>page.items),next,busy:!current||state.busy,error:current?state.error:'',reload:()=>load(),more:()=>next===null?Promise.resolve():load(next)};
}
