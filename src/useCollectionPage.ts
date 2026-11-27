import {useEffect,useMemo,useRef,useState} from 'react';
import {readApi,type Work} from './api';
import type {Filters} from './library';
import {retryWhileBusy,useCursorPage} from './useCursorPage';
export {isBusy,retryWhileBusy} from './useCursorPage';

export type CollectionQuery={filters:Filters;query:string;status:string;sort:string;titleMode:'display'|'original';locale:string};
export type CollectionPage={items:Work[];next:string|null;total:number;filtered:number;offset:number;revision:number;library_id:string;view_token:string;options:Record<'studio'|'year'|'language'|'tag',{value:string;label:string}[]>;tag_counts:Record<string,number>;studio_counts?:Record<string,number>;year_counts?:Record<string,number>;language_counts?:Record<string,number>;availability_counts?:Record<string,number>;rating_counts?:Record<string,number>;favorite_count?:number;status_counts:Record<string,number>;custom_tags:{id:string;name:string}[]};

function useCollectionStream(path:string|null,generation:number,staleMessage:string){
 const [retry,setRetry]=useState(0);
 const [state,setState]=useState<{key:string|null;pages:CollectionPage[];loadingMore:boolean;error:string;restarted:boolean}>({key:null,pages:[],loadingMore:false,error:'',restarted:false});
 const saved=useRef<{key:string|null;pages:CollectionPage[];loadingMore:boolean}>({key:null,pages:[],loadingMore:false});
 const moreController=useRef<AbortController|null>(null);
 useEffect(()=>{
  moreController.current?.abort();moreController.current=null;
  if(!path){saved.current={key:null,pages:[],loadingMore:false};return;}
  const samePath=saved.current.key===path;
  const targetCount=samePath?Math.max(1,saved.current.pages.length):1;
  if(!samePath)saved.current={key:path,pages:[],loadingMore:false};
  const controller=new AbortController();
  const timer=setTimeout(()=>{
   void (async()=>{
    try{
     const pages:CollectionPage[]=[];let cursor:string|null=null;
     do{
      const page=await retryWhileBusy(()=>readApi<CollectionPage>(path+(cursor?'&before='+encodeURIComponent(cursor):''),controller.signal),controller.signal);
      if(controller.signal.aborted)return;
      pages.push(page);cursor=page.next;
     }while(cursor&&pages.length<targetCount);
     if(controller.signal.aborted)return;
     saved.current={key:path,pages,loadingMore:false};
     setState({key:path,pages,loadingMore:false,error:'',restarted:false});
    }catch(error){
     if(controller.signal.aborted)return;
     const message=error instanceof Error?error.message:String(error);
     if(message.startsWith(staleMessage)){
      try{
       const first=await retryWhileBusy(()=>readApi<CollectionPage>(path,controller.signal),controller.signal);
       if(controller.signal.aborted)return;
       saved.current={key:path,pages:[first],loadingMore:false};
       setState({key:path,pages:[first],loadingMore:false,error:'',restarted:true});
       return;
      }catch(retryError){
       if(controller.signal.aborted)return;
       const retryMessage=retryError instanceof Error?retryError.message:String(retryError);
       setState(old=>({key:path,pages:old.key===path?old.pages:[],loadingMore:false,error:retryMessage,restarted:false}));
       return;
      }
     }
     setState(old=>({key:path,pages:old.key===path?old.pages:[],loadingMore:false,error:message,restarted:old.key===path&&old.restarted}));
    }
   })();
  },samePath?0:200);
  return()=>{clearTimeout(timer);controller.abort();moreController.current?.abort();moreController.current=null;};
 },[path,generation,retry,staleMessage]);
 const current=path!==null&&state.key===path&&state.pages.length>0;
 const pages=current?state.pages:[];
 const last=pages.at(-1)||null;
 const data=useMemo(()=>last?{...last,items:pages.flatMap(p=>p.items),offset:0}:null,[pages,last]);
 const metadata=data||state.pages.at(-1)||null;
 function loadMore(){
  if(!path||saved.current.key!==path||saved.current.loadingMore)return;
  const cursor=saved.current.pages.at(-1)?.next;
  if(!cursor)return;
  moreController.current?.abort();
  const controller=new AbortController();moreController.current=controller;
  saved.current={...saved.current,loadingMore:true};
  setState(old=>old.key===path?{...old,loadingMore:true,error:''}:old);
  void retryWhileBusy(()=>readApi<CollectionPage>(path+'&before='+encodeURIComponent(cursor),controller.signal),controller.signal).then(nextPage=>{
   if(controller.signal.aborted||saved.current.key!==path)return;
   const nextPages=[...saved.current.pages,nextPage];
   saved.current={key:path,pages:nextPages,loadingMore:false};
   setState({key:path,pages:nextPages,loadingMore:false,error:'',restarted:false});
  }).catch(error=>{
   if(controller.signal.aborted||saved.current.key!==path)return;
   const message=error instanceof Error?error.message:String(error);
   if(message.startsWith(staleMessage)){
    saved.current={key:path,pages:[],loadingMore:false};
    void retryWhileBusy(()=>readApi<CollectionPage>(path,controller.signal),controller.signal).then(first=>{
     if(controller.signal.aborted||saved.current.key!==path)return;
     saved.current={key:path,pages:[first],loadingMore:false};
     setState({key:path,pages:[first],loadingMore:false,error:'',restarted:true});
    }).catch(retryError=>{
     if(controller.signal.aborted||saved.current.key!==path)return;
     setState(old=>old.key===path?{...old,loadingMore:false,error:retryError instanceof Error?retryError.message:String(retryError)}:old);
    });
    return;
   }
   saved.current={...saved.current,loadingMore:false};
   setState(old=>old.key===path?{...old,loadingMore:false,error:message}:old);
  });
 }
 return {data,error:path!==null&&state.key===path?state.error:'',loading:path!==null&&!current&&!(state.key===path&&state.error),loadingMore:current&&state.loadingMore,metadata,restarted:path!==null&&state.key===path&&state.restarted,reload:()=>setRetry(n=>n+1),next:loadMore,previous:()=>{},loadMore,pageIndex:0};
}

/** Never display a previous query's rows as the result of the current query. */
export function useCollectionPage(query:CollectionQuery,generation:number,limit=60,infinite=false){
 const params=new URLSearchParams({query:JSON.stringify(query)});
 if(limit!==60)params.set('limit',String(limit));
 const path='/collection?'+params;
 const paged=useCursorPage<CollectionPage>(infinite?null:path,generation,'Collection or filters changed');
 const stream=useCollectionStream(infinite?path:null,generation,'Collection or filters changed');
 return infinite?stream:{...paged,loadingMore:false,loadMore:()=>{}};
}
