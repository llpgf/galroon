import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
export default defineConfig({plugins:[react()],server:{port:1420,strictPort:true,watch:{ignored:['**/test-output/**','**/target/**','**/tools/**','**/.galroon/**','**/output/**']},proxy:{'/api':'http://127.0.0.1:14800'}},build:{outDir:'dist'}});
