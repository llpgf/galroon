/**
 * Side drawers (`.scrim > .drawer`) share keyboard behaviour without each one wiring it up:
 * focus moves into a drawer when it opens and returns to the control that opened it,
 * and Escape does exactly what the drawer's own close button does (nothing while that button is disabled).
 */
const FIELDS='input:not([type=hidden]):not([disabled]):not([readonly]),textarea:not([disabled]):not([readonly]),select:not([disabled])';

function topDrawer(root:Document){
 const drawers=root.querySelectorAll<HTMLElement>('.scrim>.drawer');
 return drawers[drawers.length-1]||null;
}

function modalDialogOpen(root:Document){
 // A modal <dialog> on top handles Escape itself.
 return Array.from(root.querySelectorAll('dialog[open]')).some(dialog=>{try{return dialog.matches(':modal');}catch{return true;}});
}

function focusDrawer(drawer:HTMLElement){
 if(drawer.contains(drawer.ownerDocument.activeElement))return;
 const field=Array.from(drawer.querySelectorAll<HTMLElement>(FIELDS)).find(element=>element.getClientRects().length>0);
 if(field){field.focus({preventScroll:true});return;}
 if(!drawer.hasAttribute('tabindex'))drawer.tabIndex=-1;
 drawer.focus({preventScroll:true});
}

export function installDrawerKeys(root:Document=document){
 const openers=new WeakMap<Element,HTMLElement|null>();
 let lastFocus:HTMLElement|null=null;
 const remember=(event:FocusEvent)=>{const target=event.target;if(target instanceof HTMLElement&&!target.closest('.scrim'))lastFocus=target;};
 const observer=new MutationObserver(records=>{
  for(const record of records){
   for(const node of record.addedNodes){
    if(!(node instanceof HTMLElement)||!node.matches('.scrim'))continue;
    openers.set(node,lastFocus);
    const drawer=node.querySelector<HTMLElement>(':scope>.drawer');
    if(drawer)focusDrawer(drawer);
   }
   for(const node of record.removedNodes){
    if(!(node instanceof HTMLElement)||!openers.has(node))continue;
    const opener=openers.get(node);openers.delete(node);
    const active=root.activeElement;
    if((!active||active===root.body)&&opener?.isConnected&&!opener.closest('[hidden],[inert]'))opener.focus({preventScroll:true});
   }
  }
 });
 const onKey=(event:KeyboardEvent)=>{
  if(event.key!=='Escape'||event.defaultPrevented||event.isComposing||modalDialogOpen(root))return;
  const drawer=topDrawer(root);
  const close=drawer?.querySelector<HTMLButtonElement>(':scope>.drawer-close');
  if(!close||close.disabled)return;
  event.preventDefault();
  close.click();
 };
 root.addEventListener('focusin',remember);
 root.addEventListener('keydown',onKey);
 observer.observe(root.body,{childList:true,subtree:true});
 return()=>{observer.disconnect();root.removeEventListener('focusin',remember);root.removeEventListener('keydown',onKey);};
}
