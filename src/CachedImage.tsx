import {useRef,useState,useEffect,type ReactNode} from 'react';
import {useArtwork} from './useArtwork';
export function CachedImage({source,fallback}:{source:string;fallback?:ReactNode}){
 const target=useRef<HTMLImageElement>(null),url=useArtwork(source,target);
 const [failed,setFailed]=useState(false);useEffect(()=>setFailed(false),[url]);
 return <><img ref={target} src={failed?undefined:url} alt="" style={{opacity:url&&!failed?1:0}} onError={()=>setFailed(true)}/>{(!url||failed)&&fallback}</>;
}
