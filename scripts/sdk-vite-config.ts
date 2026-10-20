import { readFileSync } from 'node:fs';
import type { UserConfig } from '../packages/viewer-core/node_modules/vite';

export function sdkConfig(format?: string): UserConfig {
  return {
    base: './',
    publicDir: false,
    plugins: [{
      name: 'sdk-project-license',
      generateBundle() {
        this.emitFile({
          type: 'asset',
          fileName: 'LICENSE',
          source: readFileSync(new URL('../LICENSE', import.meta.url), 'utf8'),
        });
      },
    }],
    // Preserve the asset boundary when a consuming Vite build processes this
    // library again (the small classic PDF worker must not become a data URL).
    experimental: {
      renderBuiltUrl(filename, { hostType }) {
        return hostType === 'js'
          ? { runtime: `new URL(${JSON.stringify(`${filename}?no-inline`)}, import.meta.url).href` }
          : { relative: true };
      },
    },
    build: {
      target: 'es2022',
      lib: { entry: 'src/index.ts', formats: ['es'], fileName: 'index' },
      rollupOptions: {
        external: id => id.startsWith('@office-view-suite/'),
        output: {
          assetFileNames: asset => {
            const name = asset.names?.[0] ?? asset.name ?? '';
            const subdir = format === 'pdf' && name.startsWith('pdfium.std') ? 'pdfium/'
              : format === 'pdf' && name.startsWith('pdfium_view_rs') ? 'glue/' : '';
            return `assets/${format}/${subdir}[name][extname]`;
          },
        },
      },
    },
  };
}
