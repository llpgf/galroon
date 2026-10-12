export type CatalogChange={seq:number;library_id:string};
// The existing jobs scheduler owns both polls: active jobs 1.3s, idle 4s, hidden 15s.
export function startCatalogPolling({jobs,changes,refresh,error,blocked,initial}:{initial?:CatalogChange;jobs:()=>Promise<boolean>;changes:()=>Promise<CatalogChange>;refresh:()=>Promise<void>;error:(e:unknown)=>void;blocked:()=>boolean}){
 let live=true,active=true,pending=false,timer:ReturnType<typeof setTimeout>|undefined,previous:CatalogChange|undefined=initial,nextChange=0;
 const visible=()=>document.visibilityState==='visible';
 const schedule=()=>{if(live){const delay=visible()?Math.min(active?1300:4000,Math.max(0,nextChange-Date.now())):15000;timer=setTimeout(()=>void tick(),delay);}};
 const tick=async()=>{
  if(blocked()||pending){nextChange=Math.max(nextChange,Date.now()+1300);schedule();return;}
  pending=true;
  try{
   // A failed jobs read must not prevent collection invalidation from being checked.
   try{active=await jobs();}catch(e){if(live)error(e);}
   if(live&&visible()&&Date.now()>=nextChange){
    nextChange=Date.now()+10000;
    const current=await changes();if(!live)return;
    if(previous&&(current.seq!==previous.seq||current.library_id!==previous.library_id))await refresh();
    previous=current;
   }
  }catch(e){if(live)error(e);}finally{pending=false;schedule();}
 };
 const wake=()=>{if(visible()&&!pending){clearTimeout(timer);schedule();}};
 schedule();document.addEventListener('visibilitychange',wake);
 return()=>{live=false;clearTimeout(timer);document.removeEventListener('visibilitychange',wake);};
}
