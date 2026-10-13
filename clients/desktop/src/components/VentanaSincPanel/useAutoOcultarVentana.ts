import { useCallback, useEffect, type Dispatch, type SetStateAction } from 'react';
import { useSyncStore } from '@app/stores/varios-4/syncStore';

/* Devuelve ocultarVentana (cierre desde el menu) y, al perder foco, oculta la ventana. */
export function useAutoOcultarVentana(
    setMenu: Dispatch<SetStateAction<{ abierto: boolean; x: number; y: number }>>,
) {
    const ocultarVentana = useCallback(async () => {
        useSyncStore.getState().cerrarPanel();
        try {
            const { getCurrentWindow } = await import('@tauri-apps/api/window');
            await getCurrentWindow().hide();
        } catch {
            /* Entorno no-Tauri */
        }
    }, []);

    /* Sincronizar estado del store y auto-hide al perder foco.
     * El backend Rust tiene un handler equivalente con 220ms delay.
     * Este handler frontend actua como fallback (ej: binario no recompilado). */
    useEffect(() => {
        let desuscribir: (() => void) | undefined;
        (async () => {
            try {
                const { getCurrentWindow } = await import('@tauri-apps/api/window');
                const ventana = getCurrentWindow();
                desuscribir = await ventana.onFocusChanged(({ payload: enfocado }) => {
                    if (!enfocado) {
                        setMenu(prev => ({ ...prev, abierto: false }));
                        /* Auto-hide: delay para no interferir con toggle del tray icon.
                         * Si el tray handler ya mostrara la ventana, isFocused() sera true
                         * y el hide no se ejecuta. */
                        setTimeout(async () => {
                            try {
                                const sigueEnfocado = await ventana.isFocused();
                                if (!sigueEnfocado) {
                                    useSyncStore.getState().cerrarPanel();
                                    await ventana.hide();
                                }
                            } catch { /* Silencioso */ }
                        }, 300);
                    } else {
                        useSyncStore.getState().abrirPanel();
                    }
                });
            } catch {
                /* Entorno no-Tauri */
            }
        })();
        return () => desuscribir?.();
    }, []);

    return ocultarVentana;
}
