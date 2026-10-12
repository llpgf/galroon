/** Web roots intentionally have no host path. */
export function SourcePath({path}:{path?:string}){return path?<p className="path">{path.replace(/^\\\\\?\\/,'')}</p>:null;}
