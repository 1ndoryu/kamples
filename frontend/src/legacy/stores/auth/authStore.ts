/*
 * Store: authStore — Kamples
 * Estado global de autenticación y usuario actual.
 */

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { UsuarioAutenticado } from '../../types';

export interface EstadoAuth {
    usuario: UsuarioAutenticado | null;
    cargando: boolean;
    autenticado: boolean;
    /*
     * QK3: Indica si los datos del usuario vienen de la API /me (fuente completa)
     * y no de un cache parcial (Tauri Store, GLORY_CONTEXT fallback).
     * Previene decisiones basadas en datos incompletos (ej: abrir modal de generos
     * porque el cache no incluia generosPreferidos).
     */
    perfilVerificado: boolean;

    /* Acciones */
    setUsuario: (usuario: UsuarioAutenticado | null, verificado?: boolean) => void;
    setCargando: (cargando: boolean) => void;
    cerrarSesion: () => void;
}

/*
 * [204A-2] persist middleware: persiste usuario+autenticado en localStorage.
 * En el SPA web, GLORY_CONTEXT.isLoggedIn siempre es false (sin PHP),
 * por lo que useInicializadorAuth detecta autenticado=true desde el store
 * persistido y salta directo a setCargando(false) sin llamar setUsuario(null).
 * cargando y perfilVerificado NO se persisten (se recalculan en cada sesión).
 */
export const useAuthStore = create<EstadoAuth>()(
    persist(
        (set) => ({
            usuario: null,
            cargando: true,
            autenticado: false,
            perfilVerificado: false,

            setUsuario: (usuario, verificado) =>
                set({
                    usuario,
                    autenticado: usuario !== null,
                    cargando: false,
                    perfilVerificado: verificado ?? (usuario !== null),
                }),

            setCargando: (cargando) => set({ cargando }),

            cerrarSesion: () =>
                set({
                    usuario: null,
                    autenticado: false,
                    cargando: false,
                    perfilVerificado: false,
                }),
        }),
        {
            name: 'kamples-auth',
            /* Solo persistir lo que necesitamos para restaurar la sesión.
             * cargando y perfilVerificado se recalculan siempre. */
            partialize: (state) => ({
                usuario: state.usuario,
                autenticado: state.autenticado,
            }),
        }
    )
);
