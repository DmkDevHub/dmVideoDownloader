import { build } from 'vite';
import { copyFileSync, mkdirSync } from 'node:fs';
mkdirSync('dist', { recursive: true });
for (const name of ['manifest.json', 'popup.html', 'popup.css']) copyFileSync(name, `dist/${name}`);
for (const [name, format] of [['background', 'es'], ['content', 'iife'], ['popup', 'es']]) {
  await build({ configFile: false, build: {
    outDir: 'dist', emptyOutDir: false, sourcemap: false,
    lib: { entry: `src/${name}.ts`, formats: [format], name: 'DmVideo', fileName: () => `${name}.js` },
  } });
}
