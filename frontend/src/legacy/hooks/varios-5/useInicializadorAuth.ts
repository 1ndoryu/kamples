/*
 * Hook: useInicializadorAuth
 * Lógica extraída de InicializadorAuth (SRP).
 * Lee GLORY_CONTEXT para detectar sesión de WordPress y sincroniza el authStore.
 * Preserva patrón cancelado/cleanup de sesión anterior.
 * [256A-1c] Cuando autenticado=true desde store persistido, verifica /me para
 * detectar backend caído. Si falla, limpia auth para mostrar login en vez de
 * pantalla rota sin datos.
 */

import { useEffect } from 'react';
import { useAuthStore } from '@app/stores/auth/authStore';
import { obtenerUsuarioActual } from '@app/services/api/varios-1/apiAuth';
import { crearLogger } from '@app/services/varios-1/logger';
import { esNativo } from '@app/utils/varios-1/plataforma';
import { esEscritorio } from '@app/utils/varios-1/plataforma';

const log = crearLogger('InicializadorAuth');
const LS_KEY_TOKEN = 'kamples_auth_token';

interface GloryContextUser {
    id: number;
    username: string;
    email: string;
    nombreVisible: string;
    avatarUrl: string | null;
}

interface GloryContext {
    isLoggedIn?: boolean;
    userId?: number;
    currentUser?: GloryContextUser;
}

const obtenerContexto = (): GloryContext | null => {
    return (window as unknown as Record<string, unknown>).GLORY_CONTEXT as GloryContext | null;
};

export const useInicializadorAuth = (): void => {
    const autenticado = useAuthStore(s => s.autenticado);
    const perfilVerificado = useAuthStore(s => s.perfilVerificado);
    const setUsuario = useAuthStore(s => s.setUsuario);
    const setCargando = useAuthStore(s => s.setCargando);

    useEffect(() => {
        let cancelado = false;
        const inicializar = async () => {
            const ctx = obtenerContexto();

            /* Si PHP dice que no hay sesión, verificar token nativo antes de cerrar */
            if (!ctx?.isLoggedIn || !ctx.userId) {
                /* [183A-41] En Capacitor/Tauri, la sesión PHP puede expirar entre reinicios
                 * pero el JWT persiste en localStorage. Intentar restaurar sesión desde el
                 * token antes de mostrar el modal de login. apiCliente ya adjunta el Bearer
                 * automáticamente en plataformas nativas. */
                if (esNativo()) {
                    try {
                        const tokenGuardado = localStorage.getItem(LS_KEY_TOKEN);
                        if (tokenGuardado) {
                            const resp = await obtenerUsuarioActual();
                            if (cancelado) return;
                            if (resp.ok && resp.data) {
                                setUsuario(resp.data);
                                /* Sincronizar GLORY_CONTEXT para evitar desync */
                                const ctxMutable = window.GLORY_CONTEXT as Record<string, unknown> | undefined;
                                if (ctxMutable) {
                                    ctxMutable.isLoggedIn = true;
                                    ctxMutable.userId = resp.data.wpUserId ?? resp.data.id;
                                }
                                log.debug('Sesión restaurada desde token nativo persistido');
                                return;
                            }
                        }
                    } catch {
                        log.debug('Token nativo expirado o inválido, mostrando login');
                    }
                }
                if (!cancelado) setUsuario(null);
                return;
            }

            /*
             * PHP dice que hay sesión. Intentar obtener perfil completo de Kamples.
             * Si la API falla (usuario no tiene perfil Kamples aún), usar datos básicos de WP.
             */
            try {
                const resp = await obtenerUsuarioActual();
                if (cancelado) return;
                if (resp.ok && resp.data) {
                    setUsuario(resp.data);
                    log.debug('Sesión Kamples verificada', resp.data);
                    return;
                }
            } catch (err) {
                if (cancelado) return;
                log.debug('API /me no disponible, usando datos de WP');
            }

            if (cancelado) return;

            /* Fallback: usar datos inyectados por PHP */
            if (ctx.currentUser) {
                /* Normalizar avatarUrl: cadena vacía → null para que Avatar muestre iniciales */
                const avatarNormalizado = ctx.currentUser.avatarUrl?.trim() || null;
                setUsuario({
                    id: ctx.currentUser.id,
                    wpUserId: ctx.currentUser.id,
                    username: ctx.currentUser.username,
                    email: ctx.currentUser.email,
                    nombreVisible: ctx.currentUser.nombreVisible,
                    avatarUrl: avatarNormalizado,
                    plan: 'free',
                    verificado: false,
                } as never, false /* QK3: datos parciales de WP, no API /me */);
                log.debug('Sesión WP detectada (sin perfil Kamples)', ctx.currentUser.username);
            } else {
                setUsuario(null);
            }
        };

        if (!autenticado) {
            inicializar();
        } else {
            setCargando(false);
            /*
             * [256A-1c] Store persistido dice autenticado=true. Verificar /me en
             * segundo plano para detectar backend caído o token inválido.
             * Si falla, limpia auth para mostrar login en vez de pantalla rota.
             * Solo en desktop/nativo — en web GLORY_CONTEXT manda.
             */
            if (esNativo() || esEscritorio()) {
                (async () => {
                    try {
                        const resp = await obtenerUsuarioActual();
                        if (cancelado) return;
                        if (!resp.ok || !resp.data) {
                            if (!cancelado) {
                                setUsuario(null);
                                /* [256A-1c] En desktop, limpiar Tauri Store y emitir logout
                                 * cross-window para que la ventana sync también se deslogee.
                                 * Sin esto, el sync panel quedaba "logeado" con token inválido. */
                                if (esEscritorio()) {
                                    import('@desktop/services/auth/authDesktopService')
                                        .then(({ cerrarSesionDesktop }) => cerrarSesionDesktop())
                                        .catch(() => { /* noop */ });
                                }
                            }
                            return;
                        }
                    } catch {
                        if (!cancelado) {
                            setUsuario(null);
                            if (esEscritorio()) {
                                import('@desktop/services/auth/authDesktopService')
                                    .then(({ cerrarSesionDesktop }) => cerrarSesionDesktop())
                                    .catch(() => { /* noop */ });
                            }
                        }
                    }
                })();
            }
        }
        return () => { cancelado = true; };
    }, []);

    /*
     * QK3: Refetch silencioso de /me cuando hay pre-autenticación
     * con datos parciales (desktop Tauri Store cache, WP fallback).
     * Obtiene datos completos (generosPreferidos, etc.) y marca perfilVerificado=true.
     * Sin esto, el modal de generos no sabe si los datos son completos.
     */
    useEffect(() => {
        if (!autenticado || perfilVerificado) return;

        let cancelado = false;
        const verificar = async () => {
            try {
                const resp = await obtenerUsuarioActual();
                if (cancelado) return;
                if (resp.ok && resp.data) {
                    setUsuario(resp.data);
                    log.debug('Perfil completo obtenido (refetch)', resp.data);
                }
            } catch {
                if (cancelado) return;
                log.debug('Refetch /me falló — datos parciales se mantienen');
            }
        };
        verificar();
        return () => { cancelado = true; };
    }, [autenticado, perfilVerificado, setUsuario]);
};
