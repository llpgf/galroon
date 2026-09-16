export type Predicate={field:string;mode:'any'|'all'|'none';values:string[];exclude:boolean;include_unknown:boolean};
export type Rule={kind:'group';join:'all'|'any';children:Rule[]}|{kind:'field';predicate:Predicate}|{kind:'year';from:number;to:number;exclude:boolean;include_unknown:boolean}|{kind:'edition';join:'all'|'any';conditions:Predicate[];exclude:boolean;include_unknown:boolean};
export type Definition={schema_version:1;scope:'collection'|'stored_references';sort:'title'|'year';descending:boolean;rule:Rule};
export type SmartList={id:string;name:string;description:string;pinned:boolean;revision:number;deleted:boolean;needs_repair:boolean;definition:Definition};
export type SmartResult={update_pending?:boolean;update_error?:string;newer_available?:boolean;computed:boolean;snapshot?:string;total?:number;created?:number;unknown_count?:number;collection_count?:number;external_count?:number;needs_repair?:boolean;definition_changed?:boolean;entries:{position:number;work_key:string;title:string}[];next_after?:number|null};
export const newCondition=():Rule=>({kind:'field',predicate:{field:'status',mode:'any',values:['backlog'],exclude:false,include_unknown:false}});
export const newDefinition=():Definition=>({schema_version:1,scope:'collection',sort:'title',descending:false,rule:{kind:'group',join:'all',children:[newCondition()]}});

export function summarizeRule(rule:Rule,t:(key:string)=>string,options:Record<string,{id:string;name:string}[]>):string{
 const flags=(text:string,exclude:boolean,unknown:boolean)=>`${exclude?t('smart_exclude')+': ':''}${text}${unknown?' · '+t('smart_include_unknown'):''}`;
 const predicate=(p:Predicate)=>flags(`${t('smartField_'+p.field)} ${t('smartMode_'+p.mode).toLowerCase()} ${p.values.map(id=>options[p.field]?.find(v=>v.id===id)?.name||t('smartUnavailableValue')).join(', ')}`,p.exclude,p.include_unknown);
 if(rule.kind==='group')return '('+rule.children.map(r=>summarizeRule(r,t,options)).join(' '+t(rule.join==='all'?'smartAnd':'smartOr')+' ')+')';
 if(rule.kind==='field')return predicate(rule.predicate);
 if(rule.kind==='year')return flags(`${t('smartYear')} ${rule.from}–${rule.to}`,rule.exclude,rule.include_unknown);
 return flags(t('smartSameEdition')+': ('+rule.conditions.map(predicate).join(' '+t(rule.join==='all'?'smartAnd':'smartOr')+' ')+')',rule.exclude,rule.include_unknown);
}
