import {reportClientError} from './clientDiagnostics';
import {invoke,isTauri} from '@tauri-apps/api/core';
export type Session={url:string;token:string;library_id?:string;device_id?:string;api_min?:number;api_max?:number};
export const isDesktop=isTauri();
let session:Session|undefined;
let epoch=0,changing=false,stopRequested=false,everConnected=false;
export type ConnectionPhase='idle'|'connecting'|'connected'|'offline'|'reconnecting'|'restoring'|'stopping'|'stopped'|'collection_changed'|'incompatible';
export type ConnectionSnapshot={phase:ConnectionPhase;error:string};
let connection:ConnectionSnapshot={phase:'idle',error:''};const listeners=new Set<()=>void>();
let retryTimer:ReturnType<typeof setTimeout>|undefined,retryDelay=500,pendingConnection:Promise<boolean>|undefined;
export const connectionSnapshot=()=>connection;
export function subscribeConnection(listener:()=>void){listeners.add(listener);return ()=>{listeners.delete(listener);};}
function phase(value:ConnectionPhase,error=''){if(error&&error!==connection.error)reportClientError(error);connection={phase:value,error};for(const listener of listeners)listener();}
function clearRetry(){if(retryTimer!==undefined){clearTimeout(retryTimer);retryTimer=undefined;}}
function scheduleReconnect(){if(!isDesktop||stopRequested||retryTimer!==undefined)return;const delay=retryDelay;retryDelay=Math.min(30000,retryDelay*2);retryTimer=setTimeout(()=>{retryTimer=undefined;void nativeConnect().catch(()=>{});},delay);}
export class SessionChangedError extends Error {constructor(){super('Collection connection changed');}}
export class CollectionChangedError extends Error {constructor(){super('A different collection or Core device answered. Open it explicitly before continuing.');}}
export class CompatibilityError extends Error {constructor(message='Core and this app do not support a common API. Update the older application before reconnecting.'){super(message);}}
function checkNative(s:Session){
 if(!s?.url||!s.token||!s.library_id||!s.device_id||typeof s.api_min!=='number'||typeof s.api_max!=='number'||s.api_min>1||s.api_max<1)throw new CompatibilityError();
}
function differs(a:Session|undefined,b:Session){return !!a&&!!a.library_id&&(a.library_id!==b.library_id||a.device_id!==b.device_id);}
function collectionChanged(){clearRetry();stopRequested=true;epoch++;phase('collection_changed');}
export class AuthenticationError extends Error {constructor(){super('Sign in again to reconnect to this collection');}}
export class ConnectionLostError extends Error {constructor(){super('The Core connection is unavailable. Reconnect before continuing.');}}
export class UncertainOperationError extends Error {constructor(){super('The connection changed before this operation was confirmed. Check Activity and the affected item before trying again; the operation was not sent again automatically.');}}
export type AccessIdentity={id:string;role:'owner'|'desktop'|'web';can_write:boolean;can_manage_access:boolean};
export const browserCoreUrl=()=>import.meta.env.DEV?'http://127.0.0.1:14800':window.location.origin;
export const coreUrl=()=>session?.url||browserCoreUrl();
export function sessionChanging(){return changing;}
function lostConnection(){if(!isDesktop||changing||stopRequested)return;if(connection.phase==='connected'){epoch++;phase('offline');}scheduleReconnect();}
function nativeConnect():Promise<boolean>{
 if(connection.phase==='collection_changed')return Promise.reject(new CollectionChangedError());
 if(pendingConnection)return pendingConnection;if(changing)return Promise.reject(new SessionChangedError());
 clearRetry();changing=true;epoch++;const attempt=epoch;phase(everConnected?'reconnecting':'connecting');
 const run=(async()=>{let failed=false,failure:unknown;try{const next=await Promise.resolve().then(()=>invoke<Session>('core_session'));checkNative(next);if(differs(session,next))throw new CollectionChangedError();if(epoch!==attempt)throw new SessionChangedError();session=next;everConnected=true;retryDelay=500;}catch(error){failed=true;failure=error;}finally{if(epoch===attempt){changing=false;epoch++;}pendingConnection=undefined;}
  if(failed){if(failure instanceof CollectionChangedError){collectionChanged();throw failure;}if(failure instanceof CompatibilityError){stopRequested=true;phase('incompatible',failure.message);throw failure;}phase('offline',failure instanceof Error?failure.message:String(failure));scheduleReconnect();throw failure;}phase('connected');return true;
 })();pendingConnection=run;return run;
}
export async function stopCore(){if(changing)throw new SessionChangedError();clearRetry();stopRequested=true;changing=true;epoch++;phase('stopping');try{await invoke('stop_core');}finally{changing=false;epoch++;phase('stopped');}}
export async function restoreCore(path:string,sha256:string){
 if(changing)throw new SessionChangedError();clearRetry();changing=true;epoch++;phase('restoring');
 try{session=await invoke<Session>('restore_core',{path,sha256});}
 catch(error){try{session=await invoke<Session>('core_session');}catch{session=undefined;}throw error;}
 finally{changing=false;epoch++;if(session){everConnected=true;phase('connected');}else{phase('offline');scheduleReconnect();}}
}
export async function connect():Promise<boolean>{if(isDesktop){stopRequested=false;return nativeConnect();}setSession({url:browserCoreUrl(),token:''});try{await api('/access/me');return true;}catch(e){if(e instanceof AuthenticationError)return false;throw e;}}
export async function selectConnection(id:string|null){
 return changeSavedConnection('select_connection',{id});
}
export async function updateConnectionEndpoint(id:string,url:string){
 return changeSavedConnection('update_connection_endpoint',{id,url});
}
async function changeSavedConnection(command:string,args:Record<string,unknown>){
 if(changing)throw new SessionChangedError();
 const previous=connection;clearRetry();stopRequested=true;changing=true;epoch++;
 try{await invoke(command,args);window.location.reload();}
 catch(error){changing=false;epoch++;stopRequested=false;phase(previous.phase,previous.error);if(previous.phase==='offline')scheduleReconnect();throw error;}
}
export async function webLogin(url:string,password:string,name:string){setSession({url,token:''});await api('/access/login',{password,name});}
export async function signOut(){try{await api('/access/logout',{});}finally{session=undefined;epoch++;}}
export function collectionStorageKey(name:string){return `galroon.${session?.library_id||'unidentified'}.${session?.device_id||'browser'}.${name}`; }
export function setSession(s:Session){clearRetry();session=s;epoch++;stopRequested=false;retryDelay=500;if(isDesktop){everConnected=true;phase('connected');}}
export function readApi<T=unknown>(path:string,signal:AbortSignal):Promise<T>{return request<T>(path,undefined,signal);}
export function api<T=unknown>(path:string,body?:unknown):Promise<T>{return request<T>(path,body);}
export async function artworkBlob(url:string,signal:AbortSignal):Promise<Blob>{
 if(signal.aborted)throw new DOMException('Request cancelled','AbortError');
 if(!session||changing)throw new SessionChangedError();
 const current=session,started=epoch;
 const controller=new AbortController(),abort=()=>controller.abort();signal.addEventListener('abort',abort,{once:true});
 const timeout=setTimeout(abort,55000);let response:Response|undefined;
 try{
 response=await fetch(`${current.url}/api/artwork?url=${encodeURIComponent(url)}`,{signal:controller.signal,credentials:current.token?'omit':'include',headers:{...(current.token?{'Authorization':`Bearer ${current.token}`}:{}),...(current.library_id?{'X-Galroon-Library':current.library_id}:{}),...(current.device_id?{'X-Galroon-Device':current.device_id}:{})}});
 const check=()=>{if(controller.signal.aborted)throw new DOMException('Request cancelled','AbortError');if(changing||started!==epoch)throw new SessionChangedError();};check();
 const library=response.headers.get('x-galroon-library'),device=response.headers.get('x-galroon-device');
 if((library&&current.library_id&&library!==current.library_id)||(device&&current.device_id&&device!==current.device_id))throw new CollectionChangedError();
 const type=response.headers.get('content-type')?.split(';')[0];
 if(!response.ok||!type||!['image/png','image/jpeg','image/webp'].includes(type)||!response.body)throw new Error('Artwork unavailable');
 const reader=response.body.getReader(),parts:Uint8Array<ArrayBuffer>[]= [];let size=0;
 try{for(;;){const item=await reader.read();if(item.done)break;check();size+=item.value.byteLength;if(size>8*1024*1024)throw new Error('Artwork exceeds size limit');parts.push(new Uint8Array(item.value));}check();return new Blob(parts,{type});}
 finally{try{await reader.cancel();}catch{/* Preserve the original read failure. */}reader.releaseLock();}
 }finally{clearTimeout(timeout);signal.removeEventListener('abort',abort);if(response?.body&&!response.body.locked){try{await response.body.cancel();}catch{/* A failed response may already be closed. */}}}
}
export function previewGrouping<T=unknown>(id:string,selection:unknown,signal:AbortSignal):Promise<T>{return request<T>(`/resources/${encodeURIComponent(id)}/regroup/preview/page`,selection,signal);}
export function previewRules<T=unknown>(definition:unknown,signal:AbortSignal):Promise<T>{return request<T>('/smart-lists/preview',definition,signal);}
export function computeRules<T=unknown>(id:string,body:{revision:number;request_id:string},signal:AbortSignal):Promise<T>{return request<T>(`/smart-lists/${encodeURIComponent(id)}/compute`,body,signal);}
async function request<T>(path:string,body?:unknown,callerSignal?:AbortSignal):Promise<T>{
 const readOnly=body===undefined||path==='/smart-lists/preview'||/^\/resources\/[^/]+\/regroup\/preview(?:\/page)?$/.test(path);
 if(callerSignal?.aborted)throw new DOMException('Request cancelled','AbortError');
 if(changing)throw new SessionChangedError();if(isDesktop&&connection.phase!=='connected')throw new ConnectionLostError();
 if(!session)throw new Error('Core is not connected');const started=epoch,current=session;
 const stale=()=>{if(changing||started!==epoch)throw readOnly?new SessionChangedError():new UncertainOperationError();};
 let response:Response,data:any;const controller=body===undefined?new AbortController():undefined;const timeout=controller?setTimeout(()=>controller.abort(),path.includes('/exploration')||path.startsWith('/people/')||path.startsWith('/characters/')||path.startsWith('/companies/')||path.startsWith('/discover?')?60000:15000):undefined;
 const abort=()=>controller?.abort();callerSignal?.addEventListener('abort',abort,{once:true});
 try{response=await fetch(`${current.url}/api${path}`,{signal:controller?.signal||callerSignal,method:body===undefined?'GET':'POST',credentials:current.token?'omit':'include',headers:{'Content-Type':'application/json',...(current.token?{'Authorization':`Bearer ${current.token}`}:{}),...(current.library_id?{'X-Galroon-Library':current.library_id}:{}),...(current.device_id?{'X-Galroon-Device':current.device_id}:{})},body:body===undefined?undefined:JSON.stringify(body)});data=await response.json();}
 catch(error){if(callerSignal?.aborted)throw error;stale();if(isDesktop){lostConnection();throw readOnly?new ConnectionLostError():new UncertainOperationError();}throw error;}finally{if(timeout!==undefined)clearTimeout(timeout);callerSignal?.removeEventListener('abort',abort);}
 stale();
 if(data?.code==='collection_changed'){collectionChanged();throw new CollectionChangedError();}
 const library=response.headers.get('x-galroon-library'),device=response.headers.get('x-galroon-device');
 if((library&&current.library_id&&library!==current.library_id)||(device&&current.device_id&&device!==current.device_id)){collectionChanged();throw readOnly?new CollectionChangedError():new UncertainOperationError();}
 if(library)current.library_id=library;if(device)current.device_id=device;
 if(response.status===401){if(isDesktop){lostConnection();throw new ConnectionLostError();}throw new AuthenticationError();}
 if(!response.ok)throw new Error(data.error||`Request failed (${response.status})`);return data as T;
}
export async function folder():Promise<string|null>{if(!isTauri())return null;const {open}=await import('@tauri-apps/plugin-dialog');return await open({directory:true,multiple:false}) as string|null;}
export const bytes=(n:number)=>{if(n===0)return '0 B';const i=Math.min(Math.floor(Math.log(n)/Math.log(1024)),4);return `${(n/1024**i).toFixed(i>0?1:0)} ${['B','KiB','MiB','GiB','TiB'][i]}`;};
export interface Work{added_order?:number;preferred_release_id?:string|null;id:string;title:string;original_title:string;vndb_id:string|null;description:string;cover:string;developer:string;released:string;tags:unknown[];aliases:string[];status:string;favorite:boolean;notes:string;revision:number;resources:number}
export interface Binding{work_id:string;release_id:string|null;role:'main'|'patch'|'extra'|'unknown'}
export interface Edition{platforms?:string[];origin?:string;notes?:string;id:string;label:string;languages:string[];edition_type:string;released_date:string;vndb_id:string|null;work_ids:string[];revision:number}
export interface Resource{primary_work_title?:string|null;unknown?:number;auto_match_reason?:string|null;root_id:string;missing:number;unverified:number;revision:number;bindings:Binding[];id:string;title:string;path:string;kind:string;work_id:string|null;release:string;files:number;bytes:number;root_path:string}
export interface Root{id:string;path:string;label:string;role:string;online:boolean;missing:number;unverified:number}
export interface JobSummary{version:1;new_works:number;new_editions:number;new_resources:number;new_files:number;updated_files:number;unmatched_resources:number;needs_review:number;skipped:number;failed:number}
export interface Job{summary?:JobSummary|null;reuse_existing?:boolean;automatic?:boolean;id:string;kind:string;state:string;processed:number;discovered:number;bytes:number;errors:number;current_path:string;message:string;created:number}
export interface Plan{created?:number;has_origin?:boolean;undo_of?:string|null;reversed_by?:string|null;id:string;kind:string;state:string;digest:string;items:{id:string;source:string;target:string;size:number}[];operations:{id:string;state:string;error:string}[]}
export interface Isolated{id:string;original:string;current:string;state:string}


export type WorkPreference={id:string;revision:number;preferred_release_id:string|null};

export type CollectionContext={library:{id:string;name:string;revision:number};core:{device:{id:string;name:string;platform:string};instance_id:string;version:string;location:string;transport:string};api:{min:number;max:number};capabilities:{resource_preview_paging?:boolean;resource_members_paging?:boolean;resource_grouping_scoped?:boolean;edition_membership_edit?:boolean;work_scoped_reads?:boolean;collection_paging?:boolean;read_catalog:boolean;edit_catalog:boolean;manage_access:boolean;file_operations:boolean;acquire_on_core:boolean;remote_acquisition:boolean;job_lifetime:string;source_availability:string;provider_status:string}};


export function requireCollectionPaging(context:Pick<CollectionContext,'capabilities'>){if(context.capabilities.collection_paging===true&&context.capabilities.work_scoped_reads===true&&context.capabilities.edition_membership_edit===true&&context.capabilities.resource_grouping_scoped===true&&context.capabilities.resource_members_paging===true&&context.capabilities.resource_preview_paging===true)return;clearRetry();stopRequested=true;const error=new CompatibilityError('This Core does not support the required collection and work queries. Update Core to the version bundled with this app, then reconnect.');phase('incompatible',error.message);throw error;}
