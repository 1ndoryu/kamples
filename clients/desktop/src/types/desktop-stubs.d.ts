/* [256A-1a] Stubs de tipos para módulos que no aplican en desktop.
 *
 * El SPA frontend (frontend/src/legacy/) contiene código portado del tema WordPress
 * que incluye imports condicionales para Capacitor (mobile), Mezclador (DAW) y otros
 * módulos que NO existen en el contexto desktop.
 *
 * Estos stubs permiten que TypeScript procese los archivos legacy sin errores.
 * Los imports son dinámicos (import()) dentro de funciones, por lo que nunca
 * se ejecutan en desktop real a menos que se llamen explícitamente.
 */

/* GloryContext — tipo global usado por TopBar.tsx */
declare interface GloryContext {
    rutaActual: string;
    autenticado: boolean;
    usuario: Record<string, unknown> | null;
    [key: string]: unknown;
}

/* Capacitor — solo aplica en mobile (Android APK)
   Estos stubs evitan errores TS sin instalar los paquetes. */
declare module '@capacitor/app' {
    export const App: {
        addListener(event: string, callback: (data: { url: string }) => void): Promise<{ remove: () => Promise<void> }>;
        getInfo(): Promise<{ version: string; build: number }>;
    };
}

declare module '@capacitor/browser' {
    export const Browser: {
        open(options: { url: string }): Promise<void>;
        close(): Promise<void>;
    };
}

declare module '@capacitor/push-notifications' {
    export interface PushNotificationToken {
        value: string;
    }
    export interface PushNotificationActionPerformed {
        actionId: string;
        inputValue?: string;
        notification: unknown;
    }
    export const PushNotifications: {
        register(): Promise<void>;
        requestPermissions(): Promise<{ granted: boolean; receive?: string }>;
        getDeliveredNotifications(): Promise<{ notifications: unknown[] }>;
        addListener(event: 'registration', callback: (token: PushNotificationToken) => void): Promise<{ remove: () => void }>;
        addListener(event: 'registrationError', callback: (error: { error: string }) => void): Promise<{ remove: () => void }>;
        addListener(event: 'pushNotificationActionPerformed', callback: (event: PushNotificationActionPerformed) => void): Promise<{ remove: () => void }>;
    };
}

declare module '@capacitor/filesystem' {
    export const Directory: {
        Data: string;
        Documents: string;
        Cache: string;
    };
    export const Filesystem: {
        readFile(options: { path: string; directory?: string }): Promise<{ data: string }>;
        writeFile(options: { path: string; data: string; directory?: string; recursive?: boolean }): Promise<{ uri: string }>;
        deleteFile(options: { path: string; directory?: string }): Promise<void>;
        mkdir(options: { path: string; directory?: string; recursive?: boolean }): Promise<void>;
        readdir(options: { path: string; directory?: string }): Promise<{ files: { name: string; type: string }[] }>;
        stat(options: { path: string; directory?: string }): Promise<{ type: string; size: number; mtime: number }>;
    };
}

/* Mezclador (DAW) — no disponible en desktop */
declare module '@mezclador/components/ErrorBoundaryMezclador' {
    export const ErrorBoundaryMezclador: React.ComponentType<{ children?: React.ReactNode }>;
}

declare module '@mezclador/components/MezcladorPanel' {
    export const MezcladorPanel: React.ComponentType<{ isOpen?: boolean; onClose?: () => void }>;
}

/* Tauri APIs — stubs para plugins Tauri que TS no resuelve */
declare module '@tauri-apps/api/app' {
    export function getVersion(): Promise<string>;
    export function getName(): Promise<string>;
    export function getTauriVersion(): Promise<string>;
}

declare module '@tauri-apps/plugin-notification' {
    export function isPermissionGranted(): Promise<boolean>;
    export function requestPermission(): Promise<'granted' | 'denied' | 'default'>;
    export function sendNotification(options: {
        title: string;
        body?: string;
        icon?: string;
        id?: number;
        channelId?: string;
    }): void;
    export function createChannel(channel: {
        id: string;
        name: string;
        importance?: number;
        description?: string;
        visibility?: number;
        vibration?: boolean;
        sound?: string;
    }): Promise<void>;
}

declare module '@tauri-apps/plugin-shell' {
    export function open(path: string): Promise<void>;
}

/* Window augmentations — propiedades específicas del proyecto
   NO declarar GLORY_CONTEXT ni __GLORY_ROUTES__ aquí;
   ya están tipados en frontend/src/glory-core/types/glory.ts */
interface Window {
    Capacitor?: {
        isNativePlatform?: () => boolean;
        getPlatform?: () => string | null;
    };
    __KAMPLES_MOBILE__?: boolean;
}
