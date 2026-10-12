import {useEffect,useState,type RefObject} from 'react';
import {artworkBlob} from './api';
// Decoded covers outlive their card: a collection refresh remounts cards, and without this every
// remounted card waited to scroll near the viewport again before reloading its cover.
const kept=new Map<string,string>(),KEEP=600;
export function keptArtwork(source:string):string|undefined{const url=kept.get(source);if(url){kept.delete(source);kept.set(source,url);}return url;}
export function keepArtwork(source:string,url:string){kept.set(source,url);while(kept.size>KEEP){const [oldest,old]=kept.entries().next().value as [string,string];kept.delete(oldest);URL.revokeObjectURL(old);}}
const loading=new Map<string,Promise<string>>();
function loadOnce(source:string):Promise<string>{
 const kept=keptArtwork(source);if(kept)return Promise.resolve(kept);
 let pending=loading.get(source);
 if(!pending){pending=artworkBlob(source,new AbortController().signal).then(blob=>{const url=URL.createObjectURL(blob);keepArtwork(source,url);return url;}).finally(()=>loading.delete(source));loading.set(source,pending);}
 return pending;
}
export function useArtwork(source:string|undefined,target?:RefObject<Element|null>){
 const [near,setNear]=useState(!target);
 useEffect(()=>{if(!target?.current)return;const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)){setNear(true);observer.disconnect();}},{rootMargin:'300px'});observer.observe(target.current);return()=>observer.disconnect();},[target]);
 const [image,setImage]=useState<{source:string;url:string}|null>(()=>{const url=source?keptArtwork(source):undefined;return source&&url?{source,url}:null;});
 useEffect(()=>{
  if(!source){setImage(null);return;}
  const cached=keptArtwork(source);if(cached){setImage({source,url:cached});return;}
  setImage(null);if(!near&&!loading.has(source))return;let active=true;
  // A refresh can remount the card mid-download; finish into the shared cache so the remount shows it at once.
  void loadOnce(source).then(url=>{if(active)setImage({source,url});}).catch(()=>{/* Keep the existing initials or placeholder. */});
  return()=>{active=false;};
 },[source,near]);
 return image&&image.source===source?image.url:undefined;
}
