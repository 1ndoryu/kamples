import { useEffect, useRef } from 'react';
import { useSyncStore } from '@app/stores/varios-4/syncStore';
import { obtenerSync } from './accesoSync';
import { historialCambioVisible, historialSamplesCambioVisible } from './historial/comparadoresHistorial';

/* Refresco en vivo: historial/colecciones sin recargar ventana */
export function usePollingHistorial(): void {
    const panelAbierto = useSyncStore(s => s.panelAbierto);
    const historial = useSyncStore(s => s.historial);
    const historialSamples = useSyncStore(s => s.historialSamples);
    const setHistorial = useSyncStore(s => s.setHistorial);
    const setHistorialSamples = useSyncStore(s => s.setHistorialSamples);
    const setColecciones = useSyncStore(s => s.setColecciones);

    /* Usa ref para la comparación de cambio, evitando que 'historial' en deps
     * recree el interval en cada actualización. Refs por instancia del hook. */
    const historialRef = useRef(historial);
    historialRef.current = historial;
    const historialSamplesRef = useRef(historialSamples);
    historialSamplesRef.current = historialSamples;

    useEffect(() => {
        if (!panelAbierto) return;

        const srv = obtenerSync();
        if (!srv?.obtenerHistorialSync) return;

        const refrescar = () => {
            try {
                /* Cross-window: re-leer datos del Tauri Store compartido.
                 * recargarHistorialDesdeStore tiene throttle interno de 5s,
                 * llamarlo cada 1.2s es seguro — solo ejecuta cada 5s. */
                if (srv.recargarHistorialDesdeStore) {
                    srv.recargarHistorialDesdeStore().catch(() => {});
                }

                /* Rehidratar portadas que llegan tarde desde pipeline backend.
                 * Tiene throttle interno (5s), así que llamarlo en polling es seguro. */
                if (srv.rehidratarImagenesPendientesSync) {
                    srv.rehidratarImagenesPendientesSync().catch(() => {});
                }

                const nuevoHistorial = srv.obtenerHistorialSync?.(50) ?? [];
                if (historialCambioVisible(historialRef.current, nuevoHistorial)) {
                    setHistorial(nuevoHistorial);
                }

                /* Refrescar historial per-sample v2 */
                if (srv.obtenerHistorialSamplesSync) {
                    const nuevoSamples = srv.obtenerHistorialSamplesSync(50);
                    if (historialSamplesCambioVisible(historialSamplesRef.current, nuevoSamples)) {
                        setHistorialSamples(nuevoSamples);
                    }
                }

                if (srv.obtenerColeccionesSync) {
                    setColecciones(srv.obtenerColeccionesSync());
                }
            } catch {
                /* Ignorar errores puntuales de refresco */
            }
        };

        const intervalo = setInterval(refrescar, 1200);
        return () => clearInterval(intervalo);
    }, [panelAbierto, setHistorial, setHistorialSamples, setColecciones]);
}
