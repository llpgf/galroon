import type {Character,Related} from './exploration';
export type DiscoveryQuery={kind:'trait'|'released'|'title'|'studio';value:string;label:string};
export type DiscoveryWork=Related&{developers?:{id:string;name:string}[]};
export type DiscoveryCharacter=Character&{vns?:{id:string;title?:string;spoiler?:number}[]};
export type DiscoveryData={results:(DiscoveryWork|DiscoveryCharacter)[];more:boolean;page:number;stale?:boolean;partial?:boolean;local_relationships_incomplete?:boolean;continuation_verified?:boolean;relationship_revision?:number;warning?:string};
export function discoveryPath(query:DiscoveryQuery,target:'works'|'characters',page:number,spoilers:boolean,revision?:number){
 const params=new URLSearchParams({kind:query.kind,value:query.value,target,page:String(page),spoilers:String(spoilers)});if(revision!==undefined)params.set('revision',String(revision));return '/discover?'+params;
}
export function mergeResults<T extends {id:string}>(previous:T[],incoming:T[]):T[]{
 const entries=new Map(previous.map(item=>[item.id,item]));for(const item of incoming)entries.set(item.id,item);return [...entries.values()];
}
export function searchableDate(value:string|undefined|null){return !!value&&/^\d{4}(?:-\d{2}){0,2}$/.test(value);}

export function discoverableCharacter(character:Character,spoilers:boolean){return spoilers||character.vns?.some(v=>v.spoiler===0)===true;}

export function manualDiscoveryPath(query:DiscoveryQuery,target:'works'|'characters',spoilers:boolean,after:string|null=null,revision?:number){const params=new URLSearchParams({kind:query.kind,value:query.value,target,spoilers:String(spoilers)});if(after)params.set('after',after);if(revision!==undefined)params.set('revision',String(revision));return '/discover/manual?'+params;}
/** Preserve provider rank and full rows, then append new manual matches by identity. */
export function mergeDiscoveryResults(provider:DiscoveryData['results'],manual:DiscoveryWork[]):DiscoveryData['results']{const ids=new Set(provider.map(r=>r.id));return [...provider,...manual.filter(r=>!ids.has(r.id))];}
