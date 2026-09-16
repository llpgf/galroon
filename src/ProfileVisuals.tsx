import {useState,useEffect,useRef} from 'react';
import {useArtwork} from './useArtwork';
import {BookOpen} from 'lucide-react';
import {initials,imageAllowed,type Artwork} from './exploration';
export function Avatar({name,image,hidden=false}:{name:string;image?:Artwork;hidden?:boolean}){
 const target=useRef<HTMLSpanElement>(null),url=useArtwork(imageAllowed(image,hidden)?image?.url:undefined,target);
 const [failed,setFailed]=useState(false);useEffect(()=>setFailed(false),[url]);
 return <span ref={target} className="person-avatar" aria-hidden="true">{url&&!failed?<img src={url} alt="" onError={()=>setFailed(true)}/>:initials(name)}</span>;
}
export function Picture({image,hidden,name,className=''}:{image?:Artwork;hidden:boolean;name:string;className?:string}){
 const target=useRef<HTMLDivElement>(null),url=useArtwork(imageAllowed(image,hidden)?image?.url:undefined,target);
 const [failed,setFailed]=useState(false);useEffect(()=>setFailed(false),[url]);
 return <div ref={target} className={'explore-art '+className}>{url&&!failed?<img src={url} alt="" loading="lazy" onError={()=>setFailed(true)}/>:<span aria-hidden="true" title={name}><BookOpen size={40} strokeWidth={1}/></span>}</div>;
}
