import type {Work} from './api';
import {Picture} from './ProfileVisuals';
export function ListCover({keys=[],works,safe}:{keys?:string[];works:Work[];safe:boolean}){
 const members=keys.slice(0,4).map(key=>key.startsWith('local:')?works.find(w=>w.id===key.slice(6)):key.startsWith('vndb:')?works.find(w=>w.vndb_id===key.slice(5)):undefined);
 return <div className="list-cover" aria-hidden="true">{(members.length?members:[undefined]).map((w,i)=><Picture key={i} image={w?.cover?{url:w.cover}:undefined} hidden={safe} name={w?.title||''}/>)}</div>;
}
