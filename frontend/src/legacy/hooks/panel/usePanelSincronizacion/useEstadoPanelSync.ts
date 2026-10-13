import { useSyncStore } from '@app/stores/varios-4/syncStore';
import { formatearTamano, formatearTiempoRelativo } from './formatoSync';

/* Lectura del store que se expone tal cual en el panel, más los valores de presentación derivados */
export function useEstadoPanelSync() {
    const panelAbierto = useSyncStore(s => s.panelAbierto);
    const tabActual = useSyncStore(s => s.tabActual);
    const carpetaLocal = useSyncStore(s => s.carpetaLocal);
    const sincronizacionActiva = useSyncStore(s => s.sincronizacionActiva);
    const ultimaSync = useSyncStore(s => s.ultimaSync);
    const estado = useSyncStore(s => s.estado);
    const progreso = useSyncStore(s => s.progreso);
    const mensajeEstado = useSyncStore(s => s.mensajeEstado);
    const archivos = useSyncStore(s => s.archivos);
    const totalArchivos = useSyncStore(s => s.totalArchivos);
    const espacioUsado = useSyncStore(s => s.espacioUsado);
    const historial = useSyncStore(s => s.historial);
    const historialSamples = useSyncStore(s => s.historialSamples);
    const colecciones = useSyncStore(s => s.colecciones);
    const cerrarPanel = useSyncStore(s => s.cerrarPanel);

    const espacioFormateado = formatearTamano(espacioUsado);
    const ultimaSyncFormateada = ultimaSync > 0 ? formatearTiempoRelativo(ultimaSync) : 'Nunca';

    return {
        panelAbierto,
        tabActual,
        carpetaLocal,
        sincronizacionActiva,
        estado,
        progreso,
        mensajeEstado,
        archivos,
        totalArchivos,
        espacioFormateado,
        ultimaSyncFormateada,
        historial,
        historialSamples,
        colecciones,
        cerrarPanel,
    };
}
