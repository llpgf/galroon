import {invoke,isTauri} from '@tauri-apps/api/core';
import {recordBrowserError} from './browserDiagnostics';
export function reportClientError(error:unknown){
 let message:string;try{message=error instanceof Error?(error.stack||error.message):String(error);}catch{message='Unprintable client error';}
 if(!isTauri()){recordBrowserError(message);return;}
 void invoke('record_client_error',{message:message.slice(0,4000)}).catch(()=>{});
}
if(typeof window!=='undefined'){
 window.addEventListener('error',event=>reportClientError(event.error||event.message));
 window.addEventListener('unhandledrejection',event=>reportClientError(event.reason));
}
