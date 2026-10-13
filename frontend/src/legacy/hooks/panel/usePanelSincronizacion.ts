/*
 * Hook: usePanelSincronizacion (barril)
 * Lógica del panel de sincronización estilo Google Drive.
 * Compone los sub-hooks de ./usePanelSincronizacion/ y devuelve el mismo objeto de siempre.
 *
 * C358: Añadido soporte para tabs (estado/historial/colecciones),
 * historial de acciones y resync forzada.
 */

import { useEstadoPanelSync } from './usePanelSincronizacion/useEstadoPanelSync';
import { useCargaHistorialYConfig } from './usePanelSincronizacion/useCargaHistorialYConfig';
import { usePollingHistorial } from './usePanelSincronizacion/usePollingHistorial';
import { useWatchdogEstado } from './usePanelSincronizacion/useWatchdogEstado';
import { useFeedbackUploads } from './usePanelSincronizacion/useFeedbackUploads';
import { useAccionesSync } from './usePanelSincronizacion/useAccionesSync';
import { useAccionesLocales } from './usePanelSincronizacion/useAccionesLocales';

export const usePanelSincronizacion = () => {
    const vista = useEstadoPanelSync();
    useCargaHistorialYConfig();
    usePollingHistorial();
    useWatchdogEstado();
    useFeedbackUploads();
    const acciones = useAccionesSync();
    const locales = useAccionesLocales();

    return {
        ...vista,
        ...locales,
        ...acciones,
    };
};
