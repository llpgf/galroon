import {api} from './api';
export type UpdateRelease={version:string;name:string;url:string;notes:string;published_at:string|null};
export type UpdateState={current:string;install_kind:'docker'|'desktop'|'standalone';enabled:boolean;checking:boolean;last_checked:number|null;last_error:string|null;update_available:boolean;latest:UpdateRelease|null};
export const readUpdates=()=>api<UpdateState>('/updates');
export const checkUpdates=()=>api<UpdateState>('/updates/check',{});
export const setUpdateSettings=(enabled:boolean)=>api<UpdateState>('/updates/settings',{enabled});
const SKIPPED='galroon.updates.skipped';
export function skippedVersion():string|null{try{return localStorage.getItem(SKIPPED);}catch{return null;}}
export function skipVersion(version:string):void{try{localStorage.setItem(SKIPPED,version);}catch{/* This client may have storage disabled. */}}
export function shouldShowUpdate(state:UpdateState|null,skipped:string|null):boolean{return !!state?.update_available&&!!state.latest&&state.latest.version!==skipped;}
