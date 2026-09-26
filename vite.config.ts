import { defineConfig, loadEnv, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const root = fileURLToPath(new URL('.', import.meta.url));

// Dev-only: GALROON_DEV_SESSION (environment or .env.development.local) points at a session JSON printed by the ui_core example,
// so a plain browser tab can talk to a local Core the same way the desktop shell does.
function devSessionBridge(): Plugin {
  return {
    name: 'galroon-dev-session',
    apply: 'serve',
    transformIndexHtml() {
      const file = process.env.GALROON_DEV_SESSION || loadEnv('development', root, 'GALROON_').GALROON_DEV_SESSION;
      if (!file) return;
      const session = readFileSync(resolve(root, file), 'utf8').trim();
      return [{ tag: 'script', injectTo: 'head-prepend', children: `window.isTauri=true;window.__TAURI_INTERNALS__={invoke:async(c)=>{if(c==='core_session')return ${session};throw new Error('Unavailable in browser preview: '+c);}};` }];
    },
  };
}

export default defineConfig({plugins:[react(),devSessionBridge()],server:{port:1420,strictPort:true,watch:{ignored:['**/test-output/**','**/target/**','**/tools/**','**/.galroon/**','**/output/**']},proxy:{'/api':'http://127.0.0.1:14800'}},build:{outDir:'dist'}});
