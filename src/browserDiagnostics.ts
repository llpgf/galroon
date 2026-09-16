import {APP_VERSION} from './version';
/** Tab-local error log. Never sends errors to Core or a third party. */
const KEY='galroon.diagnostics.v1';
const MAX_CHARS=65536,MAX_ENTRIES=100;
type Entry={time:string;version:string;level:'error';message:string};
let memory:Entry[]=[];
let storageUnavailable=false;
let memoryDirty=false;
function scrub(value:string){return value.replace(/https?:\/\/[^\s]+/gi,'[URL omitted]').replace(/bearer\s+[^\s,;]+/gi,'Bearer [redacted]').replace(/["']?\b(password|token|secret|authorization|cookie|pairing_code|access_token|refresh_token|api_key)["']?\s*[:=]\s*(?:"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|[^\s,;]+)/gi,'$1=[redacted]').slice(0,1500);}
function read():Entry[]{
 if(memoryDirty)return memory;
 try{const raw=sessionStorage.getItem(KEY);if(!raw)return memory;
  if(raw.length>MAX_CHARS)throw new Error('Oversize diagnostics');
  const parsed:unknown=JSON.parse(raw);if(!Array.isArray(parsed))throw new Error('Invalid diagnostics');
  memory=parsed.slice(-MAX_ENTRIES).filter((v):v is Entry=>!!v&&typeof v==='object'&&typeof v.time==='string'&&typeof v.message==='string').map(v=>({time:v.time.slice(0,32),version:typeof v.version==='string'&&/^[0-9A-Za-z.+-]{1,64}$/.test(v.version)?v.version:'unknown',level:'error',message:scrub(v.message)}));
 }catch{storageUnavailable=true;}return memory;
}
export function recordBrowserError(message:string){
 try{const entries=[...read(),{time:new Date().toISOString(),version:APP_VERSION,level:'error' as const,message:scrub(message)}].slice(-MAX_ENTRIES);
  while(JSON.stringify(entries).length>MAX_CHARS)entries.shift();memory=entries;memoryDirty=true;
  try{sessionStorage.setItem(KEY,JSON.stringify(entries));storageUnavailable=false;memoryDirty=false;}catch{storageUnavailable=true;}
 }catch{/* Logging must never cause another application error. */}
}
export function browserDiagnostics(){const entries=read();return {available:true,error:storageUnavailable?'Browser storage is unavailable. Changes are kept only on this page; previously saved entries may return after reload.':null,text:entries.map(v=>JSON.stringify(v)).join('\n')};}
export function clearBrowserDiagnostics(){memory=[];memoryDirty=true;try{sessionStorage.removeItem(KEY);storageUnavailable=false;memoryDirty=false;}catch{storageUnavailable=true;}}


