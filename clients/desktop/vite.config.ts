import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'path';

/*
 * Vite config para Kamples Desktop (Tauri 2.0)
 * [256A-1a] Aliases repuntados del tema WordPress al SPA Rust.
 * @  → frontend/src/glory-core
 * @app → frontend/src/legacy
 *
 * El proxy de dev redirige /api a KAMPLES_API_TARGET (env) o
 * http://localhost:3001 por defecto (backend Rust local; 3000 es de glory-pulse).
 */

const apiTarget = process.env.KAMPLES_API_TARGET || 'http://localhost:3001';

/*
 * Cargar variables del .env del proyecto raiz (../) para reutilizar
 * GOOGLE_CLIENT_ID sin duplicar archivos de configuracion.
 * loadEnv() es la API oficial de Vite para cargar .env files.
 */
const envRaiz = loadEnv('production', resolve(__dirname, '..'), '');

export default defineConfig({
    plugins: [react(), tailwindcss()],

    /* Inyectar config publica del proyecto en el bundle (build time) */
    define: {
        '__GOOGLE_CLIENT_ID__': JSON.stringify(envRaiz.GOOGLE_CLIENT_ID || ''),
    },

    /* Tauri espera un index.html estático servido por Vite */
    root: '.',

    /*
     * [256A-1c] Servir assets del SPA Rust via symlink junctions en public/.
     * clients/desktop/public/legacy-assets → frontend/public/legacy-assets
     * clients/desktop/public/auth → frontend/public/auth
     * clients/desktop/public/landing → frontend/public/landing
     * Usamos string simple (no array) para compatibilidad con todas las
     * versiones de Vite 6+. Los junctions se crean en el setup del workspace.
     */
    publicDir: resolve(__dirname, 'public'),

    build: {
        outDir: 'dist',
        emptyOutDir: true,
        target: ['es2021', 'chrome100', 'safari13'],
        minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
        sourcemap: !!process.env.TAURI_DEBUG,
        rollupOptions: {
            input: {
                main: resolve(__dirname, 'index.html'),
                sync: resolve(__dirname, 'sync.html'),
                config: resolve(__dirname, 'config.html'),
            },
        },
    },

    server: {
        port: 1420,
        strictPort: true,
        /* 0.0.0.0 para escuchar en TODAS las interfaces (WiFi, VPN, loopback).
         * TAURI_DEV_HOST controla QUÉ IP se le dice al emulador Android,
         * pero Vite debe estar disponible en todas para que cualquier IP funcione.
         * Setear TAURI_DEV_HOST=192.168.0.X (IP WiFi) al correr android dev. */
        host: '0.0.0.0',
        /*
         * Proxy: redirige peticiones al API target (kamples.com por defecto).
         * Elimina CORS porque las peticiones salen de Vite (mismo origen).
         * Para WP local: set KAMPLES_API_TARGET=http://glory.local
         */
        proxy: {
            /* [174A-111b] Backend Rust principal — las rutas /api/* va al servidor Axum. */
            '/api': {
                target: apiTarget,
                changeOrigin: true,
                secure: false,
            },
            /* [174A-111b] Uploads servidos por backend Rust (storage local o S3). */
            '/uploads': {
                target: apiTarget,
                changeOrigin: true,
                secure: false,
            },
            /* [256A-1e] Proxy wp-json con rewrite a /api/.
             * El frontend legacy envía /wp-json/kamples/v1/... pero el backend
             * Rust solo tiene /api/.... El rewrite traduce la ruta.
             * [256A-1l] /me → /users/me: el legacy esperaba WP REST /me,
             * pero Rust lo expone como /api/users/me. */
            '/wp-json': {
                target: apiTarget,
                changeOrigin: true,
                secure: false,
                rewrite: (path) => {
                    let p = path.replace(/^\/wp-json\/kamples\/v1/, '/api');
                    if (p === '/api/me') return '/api/users/me';
                    return p;
                },
            },
            /* Solo proxiar uploads (contenido subido por usuarios) al servidor.
             * Los assets del tema se sirven localmente via servirAssetsLocales(). */
            '/wp-content/uploads': {
                target: apiTarget,
                changeOrigin: true,
                secure: false,
            },
        },
        /*
         * [256A-1a] Permitir servir archivos del SPA Rust (frontend/src/).
         * Los paths del tema WordPress ya no son necesarios.
         * [256A-1c] Incluir frontend/public para que Vite sirva legacy-assets
         * y auth/ como estaticos en dev. Sin esto, resolverRutaAsset no puede
         * cargar imagenes legacy en Tauri dev mode.
         */
        fs: {
            allow: [
                '.',
                '..',
                '../../frontend/src',
                '../../frontend/public',
            ],
        },
        hmr: {
            /* TAURI_DEV_HOST es la IP WiFi del host (192.168.0.127) que el emulador
             * Android puede alcanzar. Usar localhost solo funciona en desktop. */
            host: process.env.TAURI_DEV_HOST ?? 'localhost',
            port: 1420,
            protocol: 'ws',
        },
    },

    resolve: {
        alias: {
            /* [256A-1a] Framework Glory — ahora apunta al SPA Rust en vez del tema WP */
            '@': resolve(__dirname, '../../frontend/src/glory-core'),
            /* [256A-1a] Código Kamples (stores, services, componentes) — ahora apunta a legacy/ */
            '@app': resolve(__dirname, '../../frontend/src/legacy'),
            /* Desktop-specific code */
            '@desktop': resolve(__dirname, 'src'),
            /* Cliente Orval compartido con la SPA Rust principal */
            '@api': resolve(__dirname, '../../frontend/src/api/generated'),
            /* Dependencias: usar node_modules local del desktop */
            'soundtouchjs': resolve(__dirname, 'node_modules/soundtouchjs'),
            /* Plugins Tauri — importados desde @app (legacy/) que necesita estos alias */
            '@tauri-apps/plugin-notification': resolve(__dirname, 'node_modules/@tauri-apps/plugin-notification'),
            '@tauri-apps/plugin-fs': resolve(__dirname, 'node_modules/@tauri-apps/plugin-fs'),
            '@tauri-apps/plugin-shell': resolve(__dirname, 'node_modules/@tauri-apps/plugin-shell'),
            '@tauri-apps/api/app': resolve(__dirname, 'node_modules/@tauri-apps/api/app'),
            /* [256A-1c] Stub para @mezclador (DAW) — no disponible en desktop */
            '@mezclador': resolve(__dirname, 'src/stubs/mezclador'),
            /* [256A-1c] Stubs para Capacitor — no instalado en desktop (solo mobile) */
            '@capacitor/app': resolve(__dirname, 'src/stubs/capacitor/app.ts'),
            '@capacitor/browser': resolve(__dirname, 'src/stubs/capacitor/browser.ts'),
            '@capacitor/push-notifications': resolve(__dirname, 'src/stubs/capacitor/push-notifications.ts'),
            '@capacitor/filesystem': resolve(__dirname, 'src/stubs/capacitor/filesystem.ts'),
        },
        dedupe: [
            'react',
            'react-dom',
            'lucide-react',
            'framer-motion',
            'zustand',
            '@editorjs/editorjs',
            '@editorjs/header',
            '@editorjs/paragraph',
            '@editorjs/list',
            '@editorjs/quote',
            '@editorjs/delimiter',
            '@editorjs/image',
            '@editorjs/embed',
            '@dnd-kit/core',
            '@dnd-kit/sortable',
            '@dnd-kit/utilities',
        ],
    },

    /* Capacitor no se usa en desktop — excluir para evitar errores de resolución */
    optimizeDeps: {
        exclude: [
            '@capacitor/core',
            '@capacitor/app',
            '@capacitor/local-notifications',
            '@codetrix-studio/capacitor-google-auth',
        ],
    },
});
