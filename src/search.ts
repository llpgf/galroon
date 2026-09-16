const fold=(s:string)=>s.normalize('NFKD').replace(/\p{M}/gu,'').toLowerCase().replace(/[^\p{L}\p{N}]+/gu,' ').trim();
function closeWord(a:string,b:string){
 const limit=a.length>=8?2:1;if(a.length<4||Math.abs(a.length-b.length)>limit)return false;
 const rows:number[][]=[Array.from({length:b.length+1},(_,i)=>i)];
 for(let i=1;i<=a.length;i++){const row=[i];for(let j=1;j<=b.length;j++){row[j]=Math.min(rows[i-1][j]+1,row[j-1]+1,rows[i-1][j-1]+(a[i-1]===b[j-1]?0:1));if(i>1&&j>1&&a[i-1]===b[j-2]&&a[i-2]===b[j-1])row[j]=Math.min(row[j],rows[i-2][j-2]+1);}rows.push(row);if(Math.min(...row)>limit)return false;}
 return rows[a.length][b.length]<=limit;
}
export function fuzzyIncludes(text:string,query:string){
 const q=fold(query),hay=fold(text);if(!q)return true;if(hay.includes(q)||hay.replaceAll(' ','').includes(q.replaceAll(' ','')))return true;
 const words=hay.split(' ');return q.split(' ').every(w=>hay.includes(w)||words.some(candidate=>closeWord(w,candidate)));
}
export function releaseMonth(date:string){const m=/^([1-9]\d{3})(?:-(\d{2}))?/.exec(date||'');return m?m[1]+(m[2]&&+m[2]>=1&&+m[2]<=12?'-'+m[2]:''):'—';}
export function readHistory(key:string):string[]{try{const raw=JSON.parse(localStorage.getItem(key)||'[]');return Array.isArray(raw)?raw.filter((s:unknown):s is string=>typeof s==='string'&&!!s.trim()&&s.length<=200).slice(0,10):[];}catch{return [];}}
export function writeHistory(key:string,values:string[]){try{localStorage.setItem(key,JSON.stringify(values));window.dispatchEvent(new Event('galroon-search-history'));}catch{/* Search remains available when browser storage is blocked. */}}
export function rememberSearch(query:string,key:string){const value=query.trim().slice(0,200);if(value)writeHistory(key,[value,...readHistory(key).filter(s=>fold(s)!==fold(value))].slice(0,10));}
