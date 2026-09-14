import { createServer } from 'vite';
import vue from '@vitejs/plugin-vue';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const server=await createServer({configFile:false,plugins:[vue()],cacheDir:join(tmpdir(),'lanchat-component-check'),server:{host:'127.0.0.1',port:1428,strictPort:true}});
await server.listen();server.printUrls();
