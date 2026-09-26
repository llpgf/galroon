import {createContext,useContext,useMemo} from 'react';
/** How provider names are shown. VNDB `name`/`title` are romanized; `original`/`alttitle` hold the native script. */
export type NameMode='romanized'|'original';
export type TitleMode='display'|'original';
export const DisplayContext=createContext<{names:NameMode;titles:TitleMode}>({names:'romanized',titles:'display'});
type Named={name:string;original?:string|null};
type Titled={title:string;alttitle?:string|null};
export function useNames(){
 const {names,titles}=useContext(DisplayContext);
 return useMemo(()=>{
  const name=(entity:Named)=>names==='original'&&entity.original?entity.original:entity.name;
  const title=(work:Titled)=>titles==='original'&&work.alttitle?work.alttitle:work.title;
  return {
   name,title,
   /** The other spelling, shown as a subtitle when it differs from the primary one. */
   altName:(entity:Named)=>{const other=names==='original'?entity.name:entity.original;return other&&other!==name(entity)?other:'';},
   altTitle:(work:Titled)=>{const other=titles==='original'?work.title:work.alttitle;return other&&other!==title(work)?other:'';},
  };
 },[names,titles]);
}
