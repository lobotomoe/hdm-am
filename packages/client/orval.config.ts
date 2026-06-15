import { defineConfig } from 'orval';

export default defineConfig({
  hdmZod: {
    input: {
      target: '../../docs/openapi.json',
    },
    output: {
      client: 'zod',
      mode: 'single',
      target: process.env.HDM_ZOD_TARGET ?? 'src/generated/zod.ts',
    },
  },
});
