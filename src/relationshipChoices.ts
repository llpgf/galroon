import {visibleCharacter,type Exploration} from './exploration';
import type {RelationChoice} from './RelationshipEditor';
export function relationshipChoices(data:Exploration|null,id:string,spoilers:boolean):RelationChoice[]{
 const characters=(data?.characters||[]).filter(c=>visibleCharacter(c,id,spoilers));
 const rows:RelationChoice[]=[
  ...(data?.vn?.staff||[]).map(p=>({kind:'people' as const,id:p.id,name:p.name})),
  ...(data?.vn?.va||[]).filter(v=>characters.some(c=>c.id===v.character.id)).map(v=>({kind:'people' as const,id:v.staff.id,name:v.staff.name})),
  ...characters.map(c=>({kind:'characters' as const,id:c.id,name:c.name})),
  ...(data?.vn?.developers||[]).map(c=>({kind:'companies' as const,id:c.id,name:c.name})),
  ...(data?.vn?.relations||[]).map(w=>({kind:'works' as const,id:w.id,name:w.title}))
 ];return [...new Map(rows.map(row=>[row.kind+':'+row.id,row])).values()];
}
