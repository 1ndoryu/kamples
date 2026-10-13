import { useEffect } from 'react';
import { useSyncStore } from '@app/stores/varios-4/syncStore';
import { obtenerSync } from './accesoSync';

/* Watchdog: si UI quedó en "sincronizando" pero el lock global ya terminó, normalizar estado */
export function useWatchdogEstado(): void {
    const panelAbierto = useSyncStore(s => s.panelAbierto);
    const estado = useSyncStore(s => s.estado);
    const setEstado = useSyncStore(s => s.setEstado);
    const setProgreso = useSyncStore(s => s.setProgreso);
    const setHistorial = useSyncStore(s => s.setHistorial);
    const setHistorialSamples = useSyncStore(s => s.setHistorialSamples);

    useEffect(() => {
        if (!panelAbierto || estado !== 'sincronizando') return;

        const srv = obtenerSync();
        if (!srv?.haySyncEnCurso) return;

        const verificarEstadoReal = () => {
            try {
                const sigueSync = srv.haySyncEnCurso?.() ?? false;
                if (sigueSync) return;

                setProgreso(100);
                setEstado('completado', 'Sincronizado');

                if (srv.obtenerHistorialSync) {
                    setHistorial(srv.obtenerHistorialSync(50));
                }
                if (srv.obtenerHistorialSamplesSync) {
                    setHistorialSamples(srv.obtenerHistorialSamplesSync(50));
                }
            } catch {
                /* Ignorar errores de verificación del lock */
            }
        };

        verificarEstadoReal();
        const intervalo = setInterval(verificarEstadoReal, 1200);
        return () => clearInterval(intervalo);
    }, [panelAbierto, estado, setEstado, setProgreso, setHistorial, setHistorialSamples]);
}
