import { useEffect } from 'react';
import { useSyncStore } from '@app/stores/varios-4/syncStore';
import { obtenerSync } from './accesoSync';

/* Carga la config guardada y el historial al abrir el panel */
export function useCargaHistorialYConfig(): void {
    const panelAbierto = useSyncStore(s => s.panelAbierto);
    const setCarpeta = useSyncStore(s => s.setCarpeta);
    const setActiva = useSyncStore(s => s.setActiva);
    const setUltimaSync = useSyncStore(s => s.setUltimaSync);
    const setHistorial = useSyncStore(s => s.setHistorial);
    const setHistorialSamples = useSyncStore(s => s.setHistorialSamples);
    const setColecciones = useSyncStore(s => s.setColecciones);

    /* Cargar config guardada al abrir el panel */
    useEffect(() => {
        if (!panelAbierto) return;
        const srv = obtenerSync();
        if (!srv) return;
        try {
            const config = srv.obtenerConfigSync();
            setCarpeta(config.carpetaLocal);
            setActiva(config.sincronizacionActiva);
            setUltimaSync(config.ultimaSync);
        } catch {
            /* Config no disponible — usar defaults del store */
        }
    }, [panelAbierto, setCarpeta, setActiva, setUltimaSync]);

    /* C358: Cargar historial al abrir panel (UI minimal no usa tabs) */
    useEffect(() => {
        if (!panelAbierto) return;
        const srv = obtenerSync();
        if (!srv) return;

        if (srv.obtenerHistorialSync) {
            setHistorial(srv.obtenerHistorialSync(50));
        }

        /* Historial per-sample v2 (principal para VentanaSincPanel) */
        if (srv.obtenerHistorialSamplesSync) {
            setHistorialSamples(srv.obtenerHistorialSamplesSync(50));
        }

        if (srv.obtenerColeccionesSync) {
            setColecciones(srv.obtenerColeccionesSync());
        }
    }, [panelAbierto, setHistorial, setHistorialSamples, setColecciones]);
}
