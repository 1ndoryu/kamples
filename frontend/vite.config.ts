import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';
import { colorsPlaceholderPlugin } from './vite-plugins/colors-placeholder.ts';

const apiTarget = process.env.KAMPLES_API_TARGET || 'http://localhost:3001';

export default defineConfig({
  plugins: [react(), colorsPlaceholderPlugin()],
  /* [204A-1] Aliases que replican exactamente los del frontend WordPress legacy.
   * Ver glorytemplate/App/React/tsconfig.json. */
  resolve: {
    alias: {
      '@app': fileURLToPath(new URL('./src/legacy', import.meta.url)),
      '@': fileURLToPath(new URL('./src/glory-core', import.meta.url)),
      '@mezclador': fileURLToPath(new URL('./src/mezclador', import.meta.url)),
      /* [266A-1] @desktop apunta a clients/desktop/src para imports dinamicos del
       * flujo de auth desktop (authDesktopService) desde el frontend web. */
      '@desktop': fileURLToPath(new URL('../clients/desktop/src', import.meta.url)),
      /* [204A-1] Stub modulos nativos (Tauri/Capacitor) para builds web.
       * El legacy importa dinamicamente estos modulos solo en desktop/movil; en web
       * Rollup necesita resolverlos a un stub para no fallar el build. */
      '@tauri-apps/api/app': fileURLToPath(new URL('./src/bootstrap/tauriShim.ts', import.meta.url)),
      '@tauri-apps/api/core': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/api/event': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/api/path': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/api/window': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/plugin-deep-link': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/plugin-dialog': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/plugin-fs': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/plugin-shell': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/plugin-notification': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@tauri-apps/plugin-store': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@capacitor/filesystem': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@capacitor/app': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@capacitor/browser': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@capacitor/core': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@capacitor/push-notifications': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
      '@capacitor/share': fileURLToPath(new URL('./src/bootstrap/tauriPluginsShim.ts', import.meta.url)),
    },
  },
  server: {
    port: 5173,
    /* Proxy API requests al backend Rust en desarrollo.
     * [por que] 3001 y no 3000: el 3000 lo reclama glory-pulse en el registro
     * de workspace-manager. Override con KAMPLES_API_TARGET si hace falta. */
    proxy: {
      '/api': {
        target: apiTarget,
        changeOrigin: true,
        /* Sin timeout para permitir uploads grandes (audio ~50MB puede tardar en procesarse) */
        proxyTimeout: 0,
        timeout: 0,
      },
      /* Proxy para archivos de audio/assets servidos por el backend Rust vía ServeDir */
      '/uploads': {
        target: apiTarget,
        changeOrigin: true,
      },
      '/swagger-ui': {
        target: apiTarget,
        changeOrigin: true,
      },
      '/api-docs': {
        target: apiTarget,
        changeOrigin: true,
      },
    },
  },
});
