import {useOwnedWorks,OwnedWorksNotice} from './useOwnedWorks';
import {useRef,useState} from 'react';
import {navigationOrigin,restoreNavigationFocus} from './navigationFocus';
import {useTranslation} from 'react-i18next';
import type {ReactNode} from 'react';
export type LocalWorkRenderer=(work:Work,onBack:()=>void,onWork:(work:Work)=>void)=>ReactNode;
import {ReferenceWork} from './ReferenceWork';
import {EntityProfile} from './EntityProfile';
import {DiscoverySearch} from './DiscoverySearch';
import type {Work} from './api';
import type {Person,Character,Company} from './exploration';
import type {DiscoveryWork,DiscoveryQuery} from './discovery';
type Frame={kind:'local';work:Work}|{kind:'reference';work:DiscoveryWork}|{kind:'search';query:DiscoveryQuery}|{kind:'person';entity:Person}|{kind:'character';entity:Character}|{kind:'company';entity:Company};
type ExplorerProps={workKey:string;title:string;safe:boolean;canWrite:boolean;onBack:()=>void;renderLocalWork:LocalWorkRenderer};
export function ListExplorer(props:ExplorerProps){
 const {t}=useTranslation();const isLocal=props.workKey.startsWith('local:'),isProvider=/^vndb:v[1-9][0-9]{0,11}$/.test(props.workKey);
 const lookup=useOwnedWorks(isLocal?[props.workKey.slice(6)]:isProvider?[props.workKey.slice(5)]:[],true,isLocal);
 if(lookup.loading||lookup.error)return <section><button onClick={props.onBack}>{t('back')}</button><h1>{props.title}</h1><OwnedWorksNotice state={lookup}/></section>;
 return <ResolvedListExplorer key={props.workKey+':'+(lookup.works[0]?.id||'reference')} {...props} works={lookup.works}/>;
}
function ResolvedListExplorer({workKey,title,works,safe,canWrite,onBack,renderLocalWork}:ExplorerProps&{works:Work[]}){
 const {t}=useTranslation();const [spoilers,setSpoilers]=useState(false);const container=useRef<HTMLDivElement>(null);
 const local=works.find(w=>workKey==='local:'+w.id||workKey==='vndb:'+w.vndb_id);
 const [frames,setFrames]=useState<{view:Frame;y:number;origin:HTMLElement|null}[]>(()=>local?[{view:{kind:'local',work:local},y:0,origin:null}]:/^vndb:v[1-9][0-9]{0,11}$/.test(workKey)?[{view:{kind:'reference',work:{id:workKey.slice(5),title}},y:0,origin:null}]:[]);
 function open(view:Frame){const y=window.scrollY,origin=navigationOrigin(container.current);setFrames(old=>[...old.map((f,i)=>i===old.length-1?{...f,y}:f),{view,y:0,origin}]);requestAnimationFrame(()=>window.scrollTo(0,0));}
 function back(){if(frames.length<=1){onBack();return;}const y=frames[frames.length-2].y,origin=frames.at(-1)?.origin||null;setFrames(old=>old.slice(0,-1));requestAnimationFrame(()=>{window.scrollTo(0,y);restoreNavigationFocus(origin,container.current);});}
 const onWork=(work:Work)=>open({kind:'local',work}),onPerson=(entity:Person)=>open({kind:'person',entity}),onCharacter=(entity:Character)=>open({kind:'character',entity}),onSearch=(query:DiscoveryQuery)=>open({kind:'search',query});
 if(!frames.length)return <section><button onClick={onBack}>{t('back')}</button><h1>{title}</h1><p>{t('listState_missing')}</p></section>;
 return <div ref={container}>{frames.map(({view},index)=><div key={index} hidden={index!==frames.length-1}>{view.kind==='local'?renderLocalWork(works.find(w=>w.id===view.work.id)||view.work,back,onWork):<><button className="back-button" onClick={back}>{t('back')}</button>{view.kind==='reference'?<ReferenceWork active={index===frames.length-1} initial={view.work} safe={safe} canWrite={canWrite} spoilers={spoilers} onSpoilers={()=>setSpoilers(!spoilers)} onCharacter={onCharacter} onPerson={onPerson} onCompany={entity=>open({kind:'company',entity})} onSearch={onSearch} onCollectionWork={onWork}/>:view.kind==='search'?<DiscoverySearch visible={index===frames.length-1} query={view.query} safe={safe} canWrite={canWrite} spoilers={spoilers} onSpoilers={()=>setSpoilers(!spoilers)} onCharacter={onCharacter} onWork={work=>open({kind:'reference',work})}/>:<EntityProfile active={index===frames.length-1} kind={view.kind} initial={view.entity} safe={safe} canWrite={canWrite} spoilers={spoilers} onSpoilers={()=>setSpoilers(!spoilers)} onWork={onWork} onPerson={onPerson} onCharacter={onCharacter} onSearch={onSearch} titleMode="display"/>}</>}</div>)}</div>;
}

