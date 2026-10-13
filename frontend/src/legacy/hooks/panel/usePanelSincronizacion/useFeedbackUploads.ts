import { useEffect } from 'react';
import { useSyncStore } from '@app/stores/varios-4/syncStore';
import { obtenerSync, obtenerUpload } from './accesoSync';

/* Feedback de uploads: conectar cola de subidas al estado del panel.
 * Cuando el watcher detecta un archivo arrastrado y lo sube,
 * el panel muestra feedback en tiempo real. */
export function useFeedbackUploads(): void {
    const setEstado = useSyncStore(s => s.setEstado);
    const setProgreso = useSyncStore(s => s.setProgreso);
    const setHistorial = useSyncStore(s => s.setHistorial);
    const setHistorialSamples = useSyncStore(s => s.setHistorialSamples);

    useEffect(() => {
        const upload = obtenerUpload();
        if (!upload) return;

        upload.onProgresoUpload((progreso) => {
            const { item, totalEnCola, posicionEnCola } = progreso;

            if (item.estado === 'subiendo') {
                setEstado('sincronizando', `Subiendo ${item.nombreArchivo}${totalEnCola > 1 ? ` (${posicionEnCola}/${totalEnCola})` : ''}`);
                setProgreso(totalEnCola > 0 ? Math.round((posicionEnCola / totalEnCola) * 100) : 50);
            } else if (item.estado === 'completado') {
                /* Refrescar historial al completar un upload */
                const srv = obtenerSync();
                if (srv?.obtenerHistorialSync) {
                    setHistorial(srv.obtenerHistorialSync(50));
                }
                if (srv?.obtenerHistorialSamplesSync) {
                    setHistorialSamples(srv.obtenerHistorialSamplesSync(50));
                }
                /* Si no hay más en cola, marcar como completado */
                if (totalEnCola <= 1) {
                    setEstado('completado', `Subido: ${item.nombreArchivo}`);
                    setProgreso(100);
                }
            } else if (item.estado === 'error') {
                setEstado('error', `Error subiendo ${item.nombreArchivo}`);
            }
        });
    }, [setEstado, setProgreso, setHistorial]);
}
