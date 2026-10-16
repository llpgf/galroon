import type {Work} from './api';
import type {Filters} from './library';
import {useCursorPage} from './useCursorPage';
export {isBusy,retryWhileBusy} from './useCursorPage';

export type CollectionQuery={filters:Filters;query:string;status:string;sort:string;titleMode:'display'|'original';locale:string};
export type CollectionPage={items:Work[];next:string|null;total:number;filtered:number;offset:number;revision:number;library_id:string;view_token:string;options:Record<'studio'|'year'|'language'|'tag',{value:string;label:string}[]>;tag_counts:Record<string,number>;studio_counts?:Record<string,number>;status_counts:Record<string,number>;custom_tags:{id:string;name:string}[]};

/** Never display a previous query's rows as the result of the current query. */
export function useCollectionPage(query:CollectionQuery,generation:number){
 return useCursorPage<CollectionPage>('/collection?'+new URLSearchParams({query:JSON.stringify(query)}),generation,'Collection or filters changed');
}
