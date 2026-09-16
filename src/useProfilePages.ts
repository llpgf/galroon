import {useEffect,useRef,useState} from 'react';
import {readApi} from './api';
import {appendWorks,type ProfileData,type Related} from './exploration';
import {manualProfilePath,validManualPage,type ManualProfilePage} from './manualProfile';
import {mergeProfileCredits} from './profileCredits';
type Provider={data:ProfileData;works:Related[]};
type Manual={data:ManualProfilePage;works:Related[];partial:boolean};
export function useProfilePages(kind:'person'|'character'|'company',id:string,spoilers:boolean,revision:number,active=true){
 const scope=JSON.stringify([kind,id,spoilers,revision]);const currentScope=useRef(scope);currentScope.current=scope;
 const generation=useRef(0),providerRequest=useRef<AbortController|null>(null),manualRequest=useRef<AbortController|null>(null);
 const [stateScope,setStateScope]=useState(scope),[provider,setProvider]=useState<Provider|null>(null),[manual,setManual]=useState<Manual|null>(null);
 const providerRef=useRef<Provider|null>(null),manualRef=useRef<Manual|null>(null);
 const [loading,setLoading]=useState(true),[manualLoading,setManualLoading]=useState(true),[error,setError]=useState(''),[manualError,setManualError]=useState('');
 const requestedPage=useRef(1);
 const endpoint={person:'people',character:'characters',company:'companies'}[kind];
 function stop(){generation.current++;providerRequest.current?.abort();manualRequest.current?.abort();providerRequest.current=null;manualRequest.current=null;}
 async function providerPage(page:number,refresh=false){
  if(providerRequest.current)return;requestedPage.current=page;const token=generation.current,controller=new AbortController();providerRequest.current=controller;setLoading(true);setError('');
  const expected=page>1?providerRef.current?.data.relationship_revision:undefined;
  try{const data=await readApi<ProfileData>(`/${endpoint}/${id}?page=${page}&spoilers=${spoilers}${refresh?'&refresh=true':''}${expected===undefined?'':`&revision=${expected}`}`,controller.signal);
   if(controller.signal.aborted||token!==generation.current||currentScope.current!==scope)return;
   if(!Array.isArray(data.works)||data.page!==page)throw new Error('listScopeIncomplete');
   if(expected!==undefined&&data.relationship_revision!==expected)throw new Error('profileRelationsChanged');
   const next={data:{...data,local_relationships_incomplete:data.local_relationships_incomplete||(page>1&&providerRef.current?.data.local_relationships_incomplete)||false},works:data.partial?providerRef.current?.works||[]:page===1?data.works:appendWorks(providerRef.current?.works||[],data.works)};providerRef.current=next;setProvider(next);
  }catch(e){if(!controller.signal.aborted&&token===generation.current&&currentScope.current===scope)setError(e instanceof Error?e.message:String(e));}
  finally{if(providerRequest.current===controller){providerRequest.current=null;setLoading(false);}}
 }
 async function manualPage(after:string|null){
  if(manualRequest.current)return;const token=generation.current,controller=new AbortController();manualRequest.current=controller;setManualLoading(true);setManualError('');
  const expected=after?manualRef.current?.data.revision:undefined;
  try{const data=validManualPage(await readApi<ManualProfilePage>(manualProfilePath(id,spoilers,after,expected),controller.signal),expected);
   if(controller.signal.aborted||token!==generation.current||currentScope.current!==scope)return;
   if(after&&data.next===after)throw new Error('listScopeIncomplete');
   const next={data,works:after?mergeProfileCredits(manualRef.current?.works||[],data.works):data.works,partial:data.partial||!!data.missing_work_ids.length||(after?manualRef.current?.partial||false:false)};manualRef.current=next;setManual(next);
  }catch(e){if(!controller.signal.aborted&&token===generation.current&&currentScope.current===scope)setManualError(e instanceof Error?e.message:String(e));}
  finally{if(manualRequest.current===controller){manualRequest.current=null;setManualLoading(false);}}
 }
 useEffect(()=>{stop();setStateScope(scope);providerRef.current=null;manualRef.current=null;setProvider(null);setManual(null);if(active){void providerPage(1);void manualPage(null);}else{setLoading(false);setManualLoading(false);}return stop;},[scope]);
 useEffect(()=>{if(!active){if(providerRequest.current)cancelProvider();if(manualRequest.current)cancelManual();}else{if(!providerRef.current&&!providerRequest.current)void providerPage(1);if(!manualRef.current&&!manualRequest.current)void manualPage(null);}},[active]);
 function cancelProvider(){providerRequest.current?.abort();providerRequest.current=null;setLoading(false);setError('profileLoadCancelled');}
 function cancelManual(){manualRequest.current?.abort();manualRequest.current=null;setManualLoading(false);setManualError('profileLoadCancelled');}
 const same=stateScope===scope,p=same?provider:null,m=same?manual:null;
 const changed=!!p&&!!m&&p.data.relationship_revision!==m.data.revision;
 const incomplete=!!p?.data.local_relationships_incomplete||!!m?.partial||changed;
 const credits=changed?[]:mergeProfileCredits(p?.works||[],m?.works||[]);
 return {cancelProvider,cancelManual,data:p?.data||null,credits,loading:!same||loading,manualLoading:!same||manualLoading,error:same?error:'',manualError:same?manualError:'',incomplete,changed,manualMore:!!m?.data.next,
  refresh:()=>{stop();void providerPage(1,true);void manualPage(null);},
  retry:()=>void providerPage(requestedPage.current,requestedPage.current===1),
  retryManual:()=>void manualPage(null),loadMore:()=>void providerPage((providerRef.current?.data.page||0)+1),loadMoreManual:()=>void manualPage(manualRef.current?.data.next||null),
 };
}
