import { useCallback } from 'react';
import { useSyncStore } from '@app/stores/varios-4/syncStore';
import type { TabSync } from '@app/stores/varios-4/syncStore';
import { obtenerSync } from './accesoSync';

/* Acciones de UI y de configuración local: tabs, carpeta, historial (sin lanzar sincronización) */
export function useAccionesLocales() {
    const setTab = useSyncStore(s => s.setTab);
    const setCarpeta = useSyncStore(s => s.setCarpeta);
    const setEstado = useSyncStore(s => s.setEstado);
    const setHistorial = useSyncStore(s => s.setHistorial);
    const setHistorialSamples = useSyncStore(s => s.setHistorialSamples);

    /* Cambiar tab activo */
    const cambiarTab = useCallback((tab: TabSync) => {
        setTab(tab);
    }, [setTab]);

    /* Abrir diálogo del sistema para elegir carpeta */
    const elegirCarpeta = useCallback(async () => {
        const srv = obtenerSync();
        if (!srv) return;
        try {
            const carpeta = await srv.elegirCarpetaSync();
            if (carpeta) setCarpeta(carpeta);
        } catch {
            setEstado('error', 'Error al elegir carpeta');
        }
    }, [setCarpeta, setEstado]);

    const abrirCarpetaSincronizacion = useCallback(async () => {
        const srv = obtenerSync();
        if (!srv?.abrirCarpetaSync) return;

        try {
            const ok = await srv.abrirCarpetaSync();
            if (!ok) {
                setEstado('error', 'No se pudo abrir la carpeta de sincronización');
            }
        } catch {
            setEstado('error', 'No se pudo abrir la carpeta de sincronización');
        }
    }, [setEstado]);

    const limpiarHistorialLocal = useCallback(async () => {
        const srv = obtenerSync();
        if (!srv?.limpiarHistorialSync) return;
        try {
            await srv.limpiarHistorialSync();
            setHistorial([]);
            setHistorialSamples([]);
        } catch {
            setEstado('error', 'Error al limpiar el historial');
        }
    }, [setHistorial, setHistorialSamples, setEstado]);

    return {
        cambiarTab,
        elegirCarpeta,
        abrirCarpetaSincronizacion,
        limpiarHistorialLocal,
    };
}
