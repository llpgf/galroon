import {beforeEach,describe,expect,it,vi} from 'vitest';
import {isValidElement,type ReactElement,type ReactNode} from 'react';
// Exercise the component's rendered forms and handlers without a browser dependency.
const hooks=vi.hoisted(()=>({values:[] as unknown[],cursor:0,effect:undefined as (()=>void|(()=>void))|undefined}));
const mock=vi.hoisted(()=>({webPair:vi.fn(),webLogin:vi.fn(),webAccessTransport:vi.fn(),api:vi.fn(),setSession:vi.fn()}));
vi.mock('react',async original=>({...await original<typeof import('react')>(),useState:(initial:unknown)=>{const i=hooks.cursor++;if(!(i in hooks.values))hooks.values[i]=initial;return [hooks.values[i],(value:unknown)=>{hooks.values[i]=value;}];},useEffect:(effect:()=>void|(()=>void))=>{hooks.effect=effect;}}));
vi.mock('react-i18next',()=>({useTranslation:()=>({t:(key:string)=>key})}));
vi.mock('./api',()=>({...mock,browserCoreUrl:()=> 'https://nas.local',ApiRequestError:class extends Error{constructor(message:string,public status:number){super(message);}}}));
import {WebLogin} from './WebLogin';
import {ApiRequestError} from './api';
type Element=ReactElement<Record<string,any>>;
function nodes(node:ReactNode):Element[]{if(Array.isArray(node))return node.flatMap(nodes);if(!isValidElement(node))return [];const el=node as Element;return [el,...nodes(el.props.children)];}
const connected=vi.fn();
function render(){hooks.cursor=0;return nodes(WebLogin({onConnected:connected}));}
function form(elements:Element[],label:string){return elements.find(el=>el.type==='form'&&nodes(el).some(child=>child.type==='label'&&Array.isArray(child.props.children)&&child.props.children[0]===label))!;}
function change(elements:Element[],label:string,value:string){const el=elements.find(el=>el.type==='label'&&Array.isArray(el.props.children)&&el.props.children[0]===label)!;nodes(el).find(child=>child.type==='input')!.props.onChange({target:{value}});}
async function settle(){for(let i=0;i<6;i++)await Promise.resolve();}
beforeEach(()=>{hooks.values=[];hooks.cursor=0;hooks.effect=undefined;vi.clearAllMocks();mock.webPair.mockResolvedValue({});mock.webLogin.mockResolvedValue({});mock.webAccessTransport.mockResolvedValue('loopback');connected.mockResolvedValue(undefined);});
describe('browser pairing form',()=>{
 it('offers password and pairing on loopback; submits only the pairing code',async()=>{let tree=render();hooks.effect!();await settle();expect(form(render(),'ownerPassword')).toBeDefined();change(tree,'pairingCode',' abcd-1234-abcd ');tree=render();form(tree,'pairingCode').props.onSubmit({preventDefault:vi.fn()});await settle();expect(mock.webPair).toHaveBeenCalledWith('https://nas.local','abcd-1234-abcd');expect(mock.webLogin).not.toHaveBeenCalled();expect(connected).toHaveBeenCalledTimes(1);});
 it('shows only pairing when transport is LAN',async()=>{mock.webAccessTransport.mockResolvedValue('lan');render();hooks.effect!();await settle();const tree=render();expect(form(tree,'pairingCode')).toBeDefined();expect(form(tree,'ownerPassword')).toBeUndefined();expect(form(tree,'accessToken')).toBeUndefined();});
 it('switches to pairing after password login gets 403',async()=>{mock.webLogin.mockRejectedValue(new ApiRequestError('Password login blocked',403));let tree=render();change(tree,'ownerPassword','password fixture');change(tree,'deviceName','Browser');tree=render();form(tree,'ownerPassword').props.onSubmit({preventDefault:vi.fn()});await settle();expect(mock.webLogin).toHaveBeenCalledWith('https://nas.local','password fixture','Browser');tree=render();expect(form(tree,'ownerPassword')).toBeUndefined();expect(tree.find(el=>el.props.role==='alert')?.props.children).toBe('webPairRequired');expect(connected).not.toHaveBeenCalled();});
 it('keeps password available on other errors and shows pairing errors',async()=>{mock.webLogin.mockRejectedValue(new ApiRequestError('Bad password',400));form(render(),'ownerPassword').props.onSubmit({preventDefault:vi.fn()});await settle();expect(form(render(),'ownerPassword')).toBeDefined();mock.webPair.mockRejectedValue(new Error('Invalid code'));form(render(),'pairingCode').props.onSubmit({preventDefault:vi.fn()});await settle();expect(render().find(el=>el.props.role==='alert')?.props.children).toBe('Invalid code');expect(connected).not.toHaveBeenCalled();});
 it('ignores a stale transport probe after cleanup',async()=>{let resolve!:(value:string)=>void;mock.webAccessTransport.mockReturnValue(new Promise<string>(r=>{resolve=r;}));render();const cleanup=hooks.effect!();if(typeof cleanup==='function')cleanup();resolve('lan');await settle();expect(form(render(),'ownerPassword')).toBeDefined();});
});
