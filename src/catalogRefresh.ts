import {api,type Root,type Plan,type Isolated,type AccessIdentity,type CollectionContext,type Work} from './api';
import type {JobPage} from './JobHistory';
import type {CatalogChange} from './catalogPolling';
export async function readCatalogView(target:string|null){
 const access=await api<AccessIdentity>('/access/me');
 const [roots,jobs,plans,isolated,changes,context,summary,work]=await Promise.all([
  api<Root[]>('/roots'),api<JobPage>('/jobs?paged=true'),access.can_write?api<Plan[]>('/plans'):Promise.resolve([]),access.can_write?api<Isolated[]>('/quarantine'):Promise.resolve([]),api<CatalogChange>('/changes'),api<CollectionContext>('/context'),api<{works:number;unassigned:number;revision:number}>('/dashboard'),target?api<Work>(`/works/${encodeURIComponent(target)}`).catch(e=>{if(e instanceof Error&&e.message==='Work not found or merged')return null;throw e;}):Promise.resolve(undefined),
 ]);
 return {roots,jobs,plans,isolated,access,changes,context,summary,work};
}
