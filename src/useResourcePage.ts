import type {Resource} from './api';
import {useCursorPage} from './useCursorPage';
export type ResourcePage={items:Resource[];next:string|null;total:number;offset:number;revision:number;library_id:string};

export function useResourcePage(query:string,status:string,generation:number,rootId='',excludeId=''){
 return useCursorPage<ResourcePage>('/resources/page?'+new URLSearchParams({query,status,root_id:rootId,exclude_id:excludeId}),generation,'Resources changed. Restart from the first page.');
}
