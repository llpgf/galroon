import {useEffect,useState,useSyncExternalStore,type RefObject} from 'react';
import {artworkBlob} from './api';
import {keptArtwork,keepArtwork,clientArtworkGeneration,subscribeClientArtwork} from './clientArtworkCache';
export {keptArtwork,keepArtwork} from './clientArtworkCache';
// Decoded covers outlive their card: a collection refresh remounts cards, and without this every
// remounted card waited to scroll near the viewport again before reloading its cover.
const loading=new Map<string,Promise<string>>();
function loadOnce(source:string):Promise<string>{
 const kept=keptArtwork(source);if(kept)return Promise.resolve(kept);
 const generation=clientArtworkGeneration(),key=JSON.stringify([generation,source]);let pending=loading.get(key);
 if(!pending){pending=artworkBlob(source,new AbortController().signal).then(blob=>{if(generation!==clientArtworkGeneration())throw new Error('Image cache cleared');const url=URL.createObjectURL(blob);keepArtwork(source,url,blob.size);return url;}).finally(()=>loading.delete(key));loading.set(key,pending);}
 return pending;
}
export function useArtwork(source:string|undefined,target?:RefObject<Element|null>){
 const generation=useSyncExternalStore(subscribeClientArtwork,clientArtworkGeneration);
 const [near,setNear]=useState(!target);
 useEffect(()=>{if(!target?.current)return;const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)){setNear(true);observer.disconnect();}},{rootMargin:'300px'});observer.observe(target.current);return()=>observer.disconnect();},[target]);
 const [image,setImage]=useState<{source:string;url:string;generation:number}|null>(()=>{const url=source?keptArtwork(source):undefined;return source&&url?{source,url,generation}:null;});
 useEffect(()=>{
  if(!source){setImage(null);return;}
  const cached=keptArtwork(source);if(cached){setImage({source,url:cached,generation});return;}
  setImage(null);if(!near&&!loading.has(JSON.stringify([generation,source])))return;let active=true;
  // A refresh can remount the card mid-download; finish into the shared cache so the remount shows it at once.
  void loadOnce(source).then(url=>{if(active)setImage({source,url,generation});}).catch(()=>{/* Keep the existing initials or placeholder. */});
  return()=>{active=false;};
 },[source,near,generation]);
 return image&&image.source===source&&image.generation===generation?image.url:undefined;
}
