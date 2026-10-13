/*
 * [204A-1] Stub minimal de window.GLORY_CONTEXT para arrancar el SPA sin WordPress.
 * El frontend legacy (App/React/) lee globals como GLORY_CONTEXT, __KAMPLES_*.
 * Aqui se definen valores por defecto antes de cargar cualquier modulo legacy.
 *
 * Importante: este modulo se importa en main.tsx ANTES que cualquier import legacy
 * para garantizar que window.GLORY_CONTEXT existe cuando los modulos lo lean.
 *
 * No redeclarar Window.GLORY_CONTEXT aqui: ya esta declarado en glory-core/types
 * (declaration merging se rompe si los tipos no coinciden). Casteamos a any.
 */

const w = window as unknown as {
    GLORY_CONTEXT?: Record<string, unknown>;
    __KAMPLES_DESKTOP__?: boolean;
};

/* [204A-2] Modo desktop: activa auth por Bearer token (localStorage) en lugar de
 * cookies WP. Sin esto, apiCliente no envía Authorization y todas las peticiones
 * autenticadas al backend Rust fallan con 401. */
w.__KAMPLES_DESKTOP__ = true;

/* Sanear estado de sesión persistido por Zustand ('kamples-auth').
 * Si hay autenticado:true en el store pero NO hay JWT en localStorage,
 * el estado es corrupto (sesión de stub, token expirado, etc.).
 * Limpiar aqui — ANTES de que Zustand lo lea — evita 401 en cascada en mount. */
(function limpiarSesionCorrupta() {
    const LS_AUTH = 'kamples-auth';
    const LS_TOKEN = 'kamples_auth_token';
    try {
        const raw = localStorage.getItem(LS_AUTH);
        if (!raw) return;
        const parsed = JSON.parse(raw) as {
            state?: { autenticado?: boolean };
        };
        const autenticado = parsed?.state?.autenticado === true;
        const token = localStorage.getItem(LS_TOKEN);
        if (autenticado && !token) {
            /* Estado corrupto: marcado como autenticado pero sin JWT.
             * Resetear a estado guest para evitar requests 401 en mount. */
            localStorage.removeItem(LS_AUTH);
        }
    } catch {
        /* JSON inválido — limpiar por completo para evitar estado indeterminado */
        localStorage.removeItem(LS_AUTH);
    }
}());

if (!w.GLORY_CONTEXT) {
    w.GLORY_CONTEXT = {
        /* URL base para las llamadas REST. El proxy wpJsonStub.ts intercepta
         * /wp-json/kamples/v1/* y reescribe a /api/* en el backend Rust. */
        apiUrl: '/wp-json',
        restUrl: '/wp-json',
        nonce: '',
        isLoggedIn: false,
        userId: 0,
    };
}

export {};
