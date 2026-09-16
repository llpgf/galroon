import type {Work,Resource,Root,Edition} from './api';
import {fuzzyIncludes} from './search';
export type Availability='available'|'missing'|'unverified'|'offline'|'unknown'|'no_resources';
export type Filters={studio:string;year:string;language:string;tag:string;availability:string;tags?:string[];excludedTags?:string[];favorites?:boolean};
export const emptyFilters:Filters={studio:'',year:'',language:'',tag:'',availability:''};
export type Entry={work:Work;studio:string;year:string;languages:string[];tags:string[];customTags?:string[];states:Set<Availability>;search:string};
export const normalized=(s:string)=>s.normalize('NFKC').trim().toLowerCase();
export function tagNames(tags:unknown[]):string[]{return [...new Set(tags.flatMap(tag=>typeof tag==='string'?[tag]:tag&&typeof tag==='object'&&'name' in tag&&typeof tag.name==='string'?[tag.name]:[]).map(s=>s.trim()).filter(Boolean))];}
export function titleFor(work:Work,mode:'display'|'original'){return mode==='original'?work.original_title||work.title:work.title||work.original_title;}
export function catalogEntries(works:Work[],resources:Resource[],roots:Root[],editions:Edition[]):Entry[]{
 const byRoot=new Map(roots.map(r=>[r.id,r]));const byWork=new Map<string,Resource[]>();
 for(const r of resources)for(const binding of r.bindings){const list=byWork.get(binding.work_id)||[];list.push(r);byWork.set(binding.work_id,list);}
 const languages=new Map<string,Set<string>>();for(const edition of editions)for(const wid of edition.work_ids){const set=languages.get(wid)||new Set<string>();for(const language of edition.languages){const value=normalized(language);if(value)set.add(value);}languages.set(wid,set);}
 return works.map(work=>{const members=byWork.get(work.id)||[];const states=new Set<Availability>();if(!members.length)states.add('no_resources');
  for(const r of members){const root=byRoot.get(r.root_id);if(!root)states.add('unknown');else if(!root.online)states.add('offline');if(r.missing>0)states.add('missing');if(r.unverified>0)states.add('unverified');if(!r.files||r.missing===undefined||r.unverified===undefined||(r.unknown||0)>0)states.add('unknown');else if(root?.online&&r.missing===0&&r.unverified===0)states.add('available');}
  const tags=tagNames(work.tags);return {work,studio:work.developer.trim(),year:/^[1-9]\d{3}/.exec(work.released)?.[0]||'',languages:[...(languages.get(work.id)||[])],tags,states,search:normalized([work.title,work.original_title,work.developer,...(work.aliases||[]),...tags].join(' '))};
 });
}
const valueMatches=(filter:string,values:string[])=>!filter||filter==='unknown'&&!values.length||values.some(value=>filter===`value:${normalized(value)}`);
export function matches(entry:Entry,filters:Filters,query:string,status:string):boolean{
 const w=entry.work;if(filters.favorites&&!w.favorite)return false;
 const selectedTags=filters.tags||[],excludedTags=selectedTags.filter(tag=>filters.excludedTags?.includes(tag)),includedTags=selectedTags.filter(tag=>!excludedTags.includes(tag));
 if(excludedTags.some(tag=>tag.startsWith('custom:')?entry.customTags?.includes(tag):valueMatches(tag,entry.tags)))return false;
 if(includedTags.length&&!includedTags.some(tag=>tag.startsWith('custom:')?entry.customTags?.includes(tag):valueMatches(tag,entry.tags)))return false;
 if(status!=='all'&&!(status==='favorite'?w.favorite:w.status===status))return false;
 if(!fuzzyIncludes(entry.search,query))return false;
 return valueMatches(filters.studio,entry.studio?[entry.studio]:[])&&valueMatches(filters.year,entry.year?[entry.year]:[])&&valueMatches(filters.language,entry.languages)&&valueMatches(filters.tag,entry.tags)&&(!filters.availability||entry.states.has(filters.availability as Availability));
}
export function facetOptions(entries:Entry[],key:'studio'|'year'|'language'|'tag'):{value:string;label:string}[]{
 const values=new Map<string,string>();for(const e of entries){const list=key==='language'?e.languages:key==='tag'?e.tags:e[key]?[e[key]]:[];for(const value of list){const norm=normalized(value);if(!values.has(norm))values.set(norm,value);}}
 return [...values].sort((a,b)=>key==='year'?b[0].localeCompare(a[0]):a[1].localeCompare(b[1])).map(([value,label])=>({value:`value:${value}`,label}));
}

