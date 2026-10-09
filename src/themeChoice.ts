/** Colour scheme preference. "system" follows the OS and keeps following it while the app is open. */
export type ThemeChoice='system'|'dark'|'light';
const KEY='galroon.theme';
const media=window.matchMedia('(prefers-color-scheme: light)');
export function storedTheme():ThemeChoice{try{const value=localStorage.getItem(KEY);return value==='dark'||value==='light'?value:'system';}catch{return 'system';}}
function resolve(choice:ThemeChoice){return choice==='system'?(media.matches?'light':'dark'):choice;}
function paint(choice:ThemeChoice){
 const theme=resolve(choice);
 document.documentElement.dataset.theme=theme;
 document.querySelector('meta[name="theme-color"]')?.setAttribute('content',theme==='light'?'#f2f2f7':'#000000');
}
export function applyTheme(choice:ThemeChoice){
 try{if(choice==='system')localStorage.removeItem(KEY);else localStorage.setItem(KEY,choice);}catch{/* The choice still applies for this session. */}
 paint(choice);
}
media.addEventListener('change',()=>paint(storedTheme()));
// Paint before React renders so a light-mode user never sees a dark flash.
paint(storedTheme());
