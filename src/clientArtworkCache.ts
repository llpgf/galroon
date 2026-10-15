// Disposable, session-scoped image bodies. The selected Core owns the permanent copy.
const kept=new Map<string,{url:string;bytes:number}>();
const LIMIT=64*1024*1024,KEEP=600;
let size=0,generation=0;
const listeners=new Set<()=>void>();
export const clientArtworkGeneration=()=>generation;
export function subscribeClientArtwork(listener:()=>void){listeners.add(listener);return()=>{listeners.delete(listener);};}
export function clientArtworkStats(){return {images:kept.size,bytes:size,limit:LIMIT};}
export function keptArtwork(source:string){const image=kept.get(source);if(image){kept.delete(source);kept.set(source,image);}return image?.url;}
export function keepArtwork(source:string,url:string,bytes=0){
 const previous=kept.get(source);if(previous){size-=previous.bytes;URL.revokeObjectURL(previous.url);kept.delete(source);}
 kept.set(source,{url,bytes});size+=bytes;
 while(kept.size>KEEP||size>LIMIT){const [key,image]=kept.entries().next().value!;kept.delete(key);size-=image.bytes;URL.revokeObjectURL(image.url);}
}
export function clearClientArtwork(){for(const image of kept.values())URL.revokeObjectURL(image.url);kept.clear();size=0;generation++;for(const listener of listeners)listener();}
