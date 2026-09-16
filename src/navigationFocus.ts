/** Remember only a control within this navigation surface, never the document body. */
export function navigationOrigin(container:HTMLElement|null):HTMLElement|null{
 const element=document.activeElement;
 return element instanceof HTMLElement&&element!==document.body&&!!container?.contains(element)?element:null;
}

/** Returning to a retained page should return keyboard users to its initiating control. */
export function restoreNavigationFocus(origin:HTMLElement|null,container:HTMLElement|null){
 if(origin?.isConnected&&!origin.closest('[hidden],[inert]')&&origin.getClientRects().length){focusVisible(origin);if(document.activeElement===origin)return;}
 const heading=Array.from(container?.querySelectorAll<HTMLElement>('h1')||[]).find(element=>!element.closest('[hidden],[inert]')&&element.getClientRects().length);
 if(heading)focusVisible(heading);
}

function focusVisible(element:HTMLElement){
 element.focus({preventScroll:true});
 const rect=element.getBoundingClientRect();
 let top=0,bottom=window.innerHeight;
 // The mobile navigation and sticky header can cover an otherwise in-viewport control.
 for(const overlay of document.querySelectorAll<HTMLElement>('.sidebar, .topbar')){
  const position=getComputedStyle(overlay).position,r=overlay.getBoundingClientRect();
  if(!['fixed','sticky'].includes(position)||r.right<=rect.left||r.left>=rect.right)continue;
  if(r.top<=0&&r.bottom<bottom/2)top=Math.max(top,r.bottom);
  if(r.bottom>=window.innerHeight&&r.top>window.innerHeight/2)bottom=Math.min(bottom,r.top);
 }
 if(rect.top<top+12)window.scrollBy({top:rect.top-top-12,behavior:'instant'});
 else if(rect.bottom>bottom-12)window.scrollBy({top:rect.bottom-bottom+12,behavior:'instant'});
}
