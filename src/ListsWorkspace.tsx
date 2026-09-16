import {useRef,useState} from 'react';import {useTranslation} from 'react-i18next';import {ListsPanel} from './ListsPanel';import {SmartListsPanel} from './SmartListsPanel';import {restoreNavigationFocus} from './navigationFocus';import {ListExplorer,type LocalWorkRenderer} from './ListExplorer';
export function ListsWorkspace(props:{canWrite:boolean;safe:boolean;renderLocalWork:LocalWorkRenderer}){
 const {t}=useTranslation(),[smart,setSmart]=useState(false),[opened,setOpened]=useState(false),[explore,setExplore]=useState<{key:string;title:string}|null>(null);
 const origin=useRef<{y:number;element:HTMLElement|null}>({y:0,element:null});
 function onOpen(key:string,title:string){origin.current={y:window.scrollY,element:document.activeElement as HTMLElement};setExplore({key,title});requestAnimationFrame(()=>window.scrollTo(0,0));}
 function back(){setExplore(null);requestAnimationFrame(()=>{window.scrollTo(0,origin.current.y);restoreNavigationFocus(origin.current.element,null);});}
 return <><div hidden={!!explore}><nav className="tabs"><button aria-pressed={!smart} onClick={()=>setSmart(false)}>{t('smartManualTab')}</button><button aria-pressed={smart} onClick={()=>{setSmart(true);setOpened(true);}}>{t('smartLists')}</button></nav><div hidden={smart}><ListsPanel {...props} onOpen={onOpen}/></div>{opened&&<div hidden={!smart}><SmartListsPanel canWrite={props.canWrite} onOpen={onOpen}/></div>}</div>{explore&&<ListExplorer renderLocalWork={props.renderLocalWork} workKey={explore.key} title={explore.title} safe={props.safe} canWrite={props.canWrite} onBack={back}/>}</>;
}
