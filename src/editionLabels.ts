import {readApi} from './api';
export async function resolveEditionLabels(ids:string[],signal:AbortSignal,allowMissing=false){
 const unique=[...new Set(ids)],labels:Record<string,string>={};let snapshot:{revision:number;library_id:string}|undefined;
 for(let start=0;start<unique.length;start+=60){const batch=unique.slice(start,start+60),params=new URLSearchParams({ids:JSON.stringify(batch)});if(snapshot){params.set('revision',String(snapshot.revision));params.set('library_id',snapshot.library_id);}
 const data=await readApi<{items:{id:string;label:string}[];missing:string[];revision:number;library_id:string}>('/releases/labels?'+params,signal);
 if(!Number.isSafeInteger(data.revision)||!data.library_id||!Array.isArray(data.items)||!Array.isArray(data.missing)||(!allowMissing&&data.missing.length)||data.items.length+data.missing.length!==batch.length||new Set([...data.items.map(r=>r.id),...data.missing]).size!==batch.length||data.missing.some(id=>!batch.includes(id))||data.items.some(r=>!batch.includes(r.id)||typeof r.label!=='string'))throw Error('Selected edition labels are unavailable. Retry before reviewing.');
 if(snapshot&&(snapshot.revision!==data.revision||snapshot.library_id!==data.library_id))throw Error('Edition labels changed. Retry before reviewing.');snapshot={revision:data.revision,library_id:data.library_id};for(const row of data.items)labels[row.id]=row.label;
 }return labels;
}
