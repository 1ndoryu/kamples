import { useEffect, useState } from 'react';
import { useAuthStore } from '@app/stores/auth/authStore';

/* Perfil del usuario para la cabecera: authStore primero, luego Tauri Store. */
export function usePerfilDesktop() {
    const [perfilDesktop, setPerfilDesktop] = useState<{
        nombre: string;
        avatarUrl: string | null;
    } | null>(null);

    /* QK77-B: Reaccionar cuando el usuario cambia (login/logout cross-window) */
    const authUsuarioId = useAuthStore(s => s.usuario?.id);

    /* Cargar perfil real desde store desktop (sync window no monta auth completa).
     * QK77-B: Se re-ejecuta cuando authUsuarioId cambia (login/logout cross-window)
     * para mantener nombre y avatar actualizados sin necesidad de reiniciar la app. */
    useEffect(() => {
        let cancelado = false;

        (async () => {
            try {
                /* Primero intentar authStore (actualizado por manejarLoginExterno) */
                const authUsuario = useAuthStore.getState().usuario;
                if (authUsuario && !cancelado) {
                    setPerfilDesktop({
                        nombre: authUsuario.nombreVisible || authUsuario.username || 'Usuario',
                        avatarUrl: authUsuario.avatarUrl ?? null,
                    });
                    return;
                }

                /* Fallback: leer directamente de Tauri Store */
                const { load } = await import('@tauri-apps/plugin-store');
                const store = await load('auth.json');
                const usuario = await store.get<Record<string, unknown>>('auth_usuario');
                if (!usuario || cancelado) return;

                const nombre =
                    (usuario.nombreVisible as string | undefined)
                    ?? (usuario.nombre_display as string | undefined)
                    ?? (usuario.username as string | undefined)
                    ?? '';

                const avatarRaw =
                    (usuario.avatarUrl as string | undefined)
                    ?? (usuario.avatar_url as string | undefined)
                    ?? null;

                setPerfilDesktop({
                    nombre: nombre || 'Usuario',
                    avatarUrl: avatarRaw && avatarRaw.trim() !== '' ? avatarRaw : null,
                });
            } catch {
                /* Si falla el store, usar fallback de GLORY_CONTEXT */
            }
        })();

        return () => {
            cancelado = true;
        };
    }, [authUsuarioId]);

    return perfilDesktop;
}
