import {beforeEach,expect,it,vi} from 'vitest';
import {isValidElement,type ReactElement,type ReactNode} from 'react';
const hooks=vi.hoisted(()=>({values:[] as unknown[],cursor:0,effect:undefined as (()=>void)|undefined}));
const mock=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('react',async original=>({...await original<typeof import('react')>(),useState:(initial:unknown)=>{const i=hooks.cursor++;if(!(i in hooks.values))hooks.values[i]=initial;return [hooks.values[i],(value:unknown)=>{hooks.values[i]=value;}];},useEffect:(effect:()=>void)=>{hooks.effect=effect;},useSyncExternalStore:()=>({phase:'repair_required'})}));
vi.mock('react-i18next',()=>({useTranslation:()=>({t:(key:string)=>key})}));
vi.mock('@tauri-apps/api/core',()=>mock);
vi.mock('./api',()=>({selectConnection:vi.fn(),updateConnectionEndpoint:vi.fn(),subscribeConnection:vi.fn(),connectionSnapshot:vi.fn()}));
import {ConnectionsPanel} from './ConnectionsPanel';
type Element=ReactElement<Record<string,any>>;
function nodes(node:ReactNode):Element[]{if(Array.isArray(node))return node.flatMap(nodes);if(!isValidElement(node))return [];const el=node as Element;return [el,...nodes(el.props.children)];}
function render(){hooks.cursor=0;return nodes(ConnectionsPanel());}
async function settle(){for(let i=0;i<8;i++)await Promise.resolve();}
beforeEach(()=>{hooks.values=[];hooks.cursor=0;mock.invoke.mockReset().mockResolvedValue({selected:null,entries:[]});});
it('keeps loopback pairing and submits the pasted LAN string through IPC',async()=>{
 render();hooks.effect!();await settle();let tree=render();expect(tree.filter(el=>el.type==='form')).toHaveLength(2);
 const label=tree.find(el=>el.type==='label'&&el.props.children[0]==='pastePairingString')!;nodes(label).find(el=>el.type==='input')!.props.onChange({target:{value:' galroon-pair:v1?fixture '}});
 tree=render();const form=tree.find(el=>el.type==='form'&&el.props['aria-label']==='pastePairingString')!;expect(nodes(form).find(el=>el.type==='button')!.props.disabled).toBe(false);
 form.props.onSubmit({preventDefault:vi.fn()});await settle();expect(mock.invoke).toHaveBeenCalledWith('pair_lan_connection',{pairing:'galroon-pair:v1?fixture'});
 expect(render().find(el=>el.type==='label'&&el.props.children[0]==='pastePairingString')!.props.children[1].props.value).toBe('');
});
it('retains the string and displays a failed pairing without changing selection',async()=>{
 render();hooks.effect!();await settle();mock.invoke.mockRejectedValue(new Error('Fingerprint mismatch'));
 let tree=render();nodes(tree.find(el=>el.type==='label'&&el.props.children[0]==='pastePairingString')!).find(el=>el.type==='input')!.props.onChange({target:{value:'pairing fixture'}});
 tree=render();tree.find(el=>el.type==='form'&&el.props['aria-label']==='pastePairingString')!.props.onSubmit({preventDefault:vi.fn()});await settle();
 expect(render().find(el=>el.props.role==='alert')!.props.children).toBe('Fingerprint mismatch');expect(render().find(el=>el.type==='label'&&el.props.children[0]==='pastePairingString')!.props.children[1].props.value).toBe('pairing fixture');
});
