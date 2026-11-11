import { defineConfig } from 'vite';
import cesium from 'vite-plugin-cesium';
import path from 'path';

export default defineConfig({
  plugins: [cesium()],
  build: {
    outDir: path.resolve(__dirname, '../crates/humboldt-server/dist'),
    emptyOutDir: true,
    chunkSizeWarningLimit: 5000,
  },
  server: {
    port: 5173,
  },
});
