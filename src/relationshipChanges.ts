import {useSyncExternalStore} from 'react';
let revision=0;
const listeners=new Set<()=>void>();
export function relationshipsChanged(){revision++;for(const listener of listeners)listener();}
export function useRelationshipRevision(){return useSyncExternalStore(listener=>{listeners.add(listener);return()=>{listeners.delete(listener);};},()=>revision,()=>0);}
