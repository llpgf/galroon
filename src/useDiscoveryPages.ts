import {useEffect,useRef,useState} from 'react';
import {readApi} from './api';import {validManualPage,type ManualProfilePage} from './manualProfile';
import {discoveryPath,manualDiscoveryPath,mergeResults,mergeDiscoveryResults,discoverableCharacter,type DiscoveryQuery,type DiscoveryData,type DiscoveryCharacter} from './discovery';
type Provider={data:DiscoveryData;items:DiscoveryData['results']};type Manual={data:ManualProfilePage;partial:boolean;items:ManualProfilePage['works']};
export function useDiscoveryPages(query:DiscoveryQuery,target:'works'|'characters',spoilers:boolean,revision:number,active:boolean){
 const scope=JSON.stringify([query.kind,query.value,target,spoilers,revision]);const current=useRef(scope);current.current=scope;
 const [stateScope,setStateScope]=useState(scope),[provider,setProvider]=useState<Provider|null>(null),[manual,setManual]=useState<Manual|null>(null);
 const p=useRef<Provider|null>(null),m=useRef<Manual|null>(null),generation=useRef(0),providerRequest=useRef<AbortController|null>(null),manualRequest=useRef<AbortController|null>(null),requestedPage=useRef(1);
 const [loading,setLoading]=useState(true),[manualLoading,setManualLoading]=useState(true),[error,setError]=useState(''),[manualError,setManualError]=useState('');
 function stop(){generation.current++;providerRequest.current?.abort();manualRequest.current?.abort();providerRequest.current=null;manualRequest.current=null;}
 async function providerPage(page:number){if(providerRequest.current)return;const token=generation.current,controller=new AbortController();providerRequest.current=controller;requestedPage.current=page;setLoading(true);setError('');const expected=page>1?p.current?.data.relationship_revision:undefined;
  try{const data=await readApi<DiscoveryData>(discoveryPath(query,target,page,spoilers,expected),controller.signal);if(controller.signal.aborted||token!==generation.current||current.current!==scope)return;
   if(!Array.isArray(data.results)||data.page!==page||typeof data.more!=='boolean'||!Number.isSafeInteger(data.relationship_revision))throw new Error('listScopeIncomplete');if(expected!==undefined&&data.relationship_revision!==expected)throw new Error('discoveryRelationsChanged');
   const rows=target==='characters'?data.results.filter(r=>discoverableCharacter(r as DiscoveryCharacter,spoilers)):data.results;
   if(page>1&&data.more&&!rows.some(row=>!p.current?.items.some(old=>old.id===row.id))&&!(rows.length===0&&data.continuation_verified))throw new Error('listScopeIncomplete');
   const next={data:{...data,stale:!!data.stale||(page>1&&!!p.current?.data.stale),partial:!!data.partial||(page>1&&!!p.current?.data.partial),warning:data.warning||(page>1?p.current?.data.warning:undefined),local_relationships_incomplete:!!data.local_relationships_incomplete||(page>1&&!!p.current?.data.local_relationships_incomplete)},items:data.partial&&p.current?p.current.items:page===1?rows:mergeResults(p.current?.items||[],rows)};p.current=next;setProvider(next);
  }catch(e){if(!controller.signal.aborted&&token===generation.current&&current.current===scope)setError(e instanceof Error?e.message:String(e));}finally{if(providerRequest.current===controller){providerRequest.current=null;setLoading(false);}}
 }
 async function manualPage(after:string|null){if(manualRequest.current)return;const token=generation.current,controller=new AbortController();manualRequest.current=controller;setManualLoading(true);setManualError('');const expected=after?m.current?.data.revision:undefined;
  try{const data=validManualPage(await readApi<ManualProfilePage>(manualDiscoveryPath(query,target,spoilers,after,expected),controller.signal),expected);if(controller.signal.aborted||token!==generation.current||current.current!==scope)return;if(after&&data.next===after)throw new Error('listScopeIncomplete');
   const next={data,items:after?mergeResults(m.current?.items||[],data.works):data.works,partial:data.partial||!!data.missing_work_ids.length||(after?!!m.current?.partial:false)};m.current=next;setManual(next);
  }catch(e){if(!controller.signal.aborted&&token===generation.current&&current.current===scope)setManualError(e instanceof Error?e.message:String(e));}finally{if(manualRequest.current===controller){manualRequest.current=null;setManualLoading(false);}}
 }
 useEffect(()=>{stop();setStateScope(scope);p.current=null;m.current=null;setProvider(null);setManual(null);setError('');setManualError('');if(active){void providerPage(1);void manualPage(null);}else{setLoading(false);setManualLoading(false);}return stop;},[scope]);
 useEffect(()=>{if(!active){if(providerRequest.current)cancelProvider();if(manualRequest.current)cancelManual();}else{if(!p.current&&!providerRequest.current)void providerPage(1);if(!m.current&&!manualRequest.current)void manualPage(null);}},[active]);
 function cancelProvider(){providerRequest.current?.abort();providerRequest.current=null;setLoading(false);setError('profileLoadCancelled');}function cancelManual(){manualRequest.current?.abort();manualRequest.current=null;setManualLoading(false);setManualError('profileLoadCancelled');}
 const same=stateScope===scope,source=same?provider:null,saved=same?manual:null;const changed=!!source&&!!saved&&source.data.relationship_revision!==saved.data.revision;
 return {scope,data:source?.data||null,items:changed?[]:mergeDiscoveryResults(source?.items||[],saved?.items||[]),loading:!same||loading,manualLoading:!same||manualLoading,error:same?error:'',manualError:same?manualError:'',changed,incomplete:!!source?.data.partial||!!source?.data.local_relationships_incomplete||!!saved?.partial||changed,manualMore:!!saved?.data.next,cancelProvider,cancelManual,
 refresh:()=>{stop();void providerPage(1);void manualPage(null);},retry:()=>void providerPage(requestedPage.current),retryManual:()=>void manualPage(null),loadMore:()=>void providerPage((p.current?.data.page||0)+1),loadMoreManual:()=>void manualPage(m.current?.data.next||null)};
}
