import {useId} from 'react';

/** Galroon emblem: luminous glass-blue rounded square with a geometric G and four-point star. */
export function BrandMark({className='brand-mark'}:{className?:string}){
 const id=useId().replace(/:/g,'');
 return <svg className={className} viewBox="0 0 256 256" aria-hidden="true" focusable="false">
  <defs>
   <linearGradient id={`${id}-bg`} x1="16%" y1="8%" x2="86%" y2="94%">
    <stop offset="0%" stopColor="#64c8ff"/>
    <stop offset="48%" stopColor="#0a84ff"/>
    <stop offset="100%" stopColor="#0050c8"/>
   </linearGradient>
   <radialGradient id={`${id}-glow`} cx="28%" cy="18%" r="65%">
    <stop offset="0%" stopColor="#ffffff" stopOpacity="0.3"/>
    <stop offset="100%" stopColor="#ffffff" stopOpacity="0"/>
   </radialGradient>
   <radialGradient id={`${id}-tint`} cx="85%" cy="88%" r="55%">
    <stop offset="0%" stopColor="#5e5ce6" stopOpacity="0.34"/>
    <stop offset="100%" stopColor="#5e5ce6" stopOpacity="0"/>
   </radialGradient>
   <linearGradient id={`${id}-rim`} x1="15%" y1="0%" x2="85%" y2="100%">
    <stop offset="0%" stopColor="#ffffff" stopOpacity="0.46"/>
    <stop offset="100%" stopColor="#ffffff" stopOpacity="0.08"/>
   </linearGradient>
  </defs>
  <rect width="256" height="256" rx="58" fill={`url(#${id}-bg)`}/>
  <rect width="256" height="256" rx="58" fill={`url(#${id}-glow)`}/>
  <rect width="256" height="256" rx="58" fill={`url(#${id}-tint)`}/>
  <rect x="2" y="2" width="252" height="252" rx="56" fill="none" stroke={`url(#${id}-rim)`} strokeWidth="3"/>
  <path d="M156 84.5A60 60 0 1 0 182 134H132" fill="none" stroke="#ffffff" strokeWidth="28" strokeLinecap="round" strokeLinejoin="round"/>
  <path d="M194 38Q194 57 218 62Q194 67 194 86Q194 67 170 62Q194 57 194 38Z" fill="#ffffff"/>
 </svg>;
}
