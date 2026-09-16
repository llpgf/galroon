import {useEffect,useState,type RefObject} from 'react';
import {artworkBlob} from './api';
export function useArtwork(source:string|undefined,target?:RefObject<Element|null>){
 const [near,setNear]=useState(!target);
 useEffect(()=>{if(!target?.current)return;const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)){setNear(true);observer.disconnect();}},{rootMargin:'300px'});observer.observe(target.current);return()=>observer.disconnect();},[target]);
 const [image,setImage]=useState<{source:string;url:string}|null>(null);
 useEffect(()=>{
  setImage(null);
  if(!source||!near)return;const controller=new AbortController();let objectUrl:string|undefined;
  void artworkBlob(source,controller.signal).then(blob=>{if(controller.signal.aborted)return;objectUrl=URL.createObjectURL(blob);setImage({source,url:objectUrl});}).catch(()=>{/* Keep the existing initials or placeholder. */});
  return()=>{controller.abort();if(objectUrl)URL.revokeObjectURL(objectUrl);};
 },[source,near]);
 return image&&image.source===source?image.url:undefined;
}
