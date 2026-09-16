export type ExactKey=
 |{kind:'company'|'character';id:string}
 |{kind:'staff';id:string;aid?:number|null;role:string;note:string}
 |{kind:'voice';id:string;character:string;alias:number;note:string}
 |{kind:'work';id:string;relation:string};
export type ExactLink={key:ExactKey;name:string;character_name:string|null;spoiler:number};
export type ExactCorrection={id:string;replaces:ExactKey|null;link:ExactLink|null};
export type ExactEdge={key:ExactKey;name:string;character_name?:string|null;local?:boolean;correction_id?:string|null;spoiler?:number|null;appearances?:{id:string;spoiler:number}[]};
export type ExactState={revision:number;corrections:ExactCorrection[];source:ExactEdge[];effective:ExactEdge[];source_unavailable:boolean;source_partial:boolean};
export type ExactHistory={items:{revision:number;created:number;before:ExactCorrection[];after:ExactCorrection[]}[];next:number|null};
export type Candidate={id:string;kind:string;name:string;original?:string;aliases?:{aid:number;name:string;latin?:string;ismain?:boolean}[];stale?:boolean;warning?:string;fetched_at:number};
export type ExactEdit={revision:number;request_id:string;corrections:ExactCorrection[]};
export type ExactPreview={preview_digest:string;before:ExactEdge[];after:ExactEdge[];shadowed_correction_ids:string[];source_unavailable:boolean;source_partial:boolean};
export function sameExactKey(a:ExactKey|null,b:ExactKey|null):boolean{
 if(!a||!b)return a===b;if(a.kind!==b.kind||a.id!==b.id)return false;
 if(a.kind==='staff'&&b.kind==='staff')return (a.aid??null)===(b.aid??null)&&a.role===b.role&&a.note===b.note;
 if(a.kind==='voice'&&b.kind==='voice')return a.character===b.character&&a.alias===b.alias&&a.note===b.note;
 if(a.kind==='work'&&b.kind==='work')return a.relation===b.relation;return true;
}
export function exactIdentity(key:ExactKey):string{
 return [key.id,...(key.kind==='staff'?[key.aid===null||key.aid===undefined?'—':`#${key.aid}`,key.role,key.note]:key.kind==='voice'?[`#${key.alias}`,key.character,key.note]:key.kind==='work'?[key.relation]:[])].filter(Boolean).join(' · ');
}
