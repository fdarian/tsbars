import { defineConfig } from 'tsdown';

export default defineConfig({
  entry: ['src/compile.ts', 'src/vite.ts', 'src/bun.ts'],
  format: ['esm'],
  dts: true,
  clean: true,
  outDir: 'dist',
  fixedExtension: false,
  deps: { neverBundle: ['bun'] },
});
