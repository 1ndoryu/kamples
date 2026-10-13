import { useCallback } from 'react';
import { useSyncStore } from '@app/stores/varios-4/syncStore';
import { crearLogger } from '../../../services/varios-1/logger';
import { obtenerSync } from './accesoSync';
import type { ProgresoSync } from './tipos';

const log = crearLogger('usePanelSincronizacion');

/* Acciones que disparan sincronización contra el servidor (sync manual, toggle, resync forzada/reforzada) */
export function useAccionesSync() {
    const carpetaLocal = useSyncStore(s => s.carpetaLocal);
    const sincronizacionActiva = useSyncStore(s => s.sincronizacionActiva);
    const setActiva = useSyncStore(s => s.setActiva);
    const setEstado = useSyncStore(s => s.setEstado);
    const setProgreso = useSyncStore(s => s.setProgreso);
    const agregarArchivo = useSyncStore(s => s.agregarArchivo);
    const actualizarArchivoEstado = useSyncStore(s => s.actualizarArchivoEstado);
    const setUltimaSync = useSyncStore(s => s.setUltimaSync);
    const setHistorial = useSyncStore(s => s.setHistorial);
    const setHistorialSamples = useSyncStore(s => s.setHistorialSamples);

    /* Helper: ejecutar sync con progreso.
     * Envuelve fnSync en try-catch para que un error siempre
     * transite el estado a 'error' y nunca deje el spinner congelado. */
    const ejecutarSyncConProgreso = useCallback(async (
        fnSync: (onProgreso?: (p: ProgresoSync) => void) => Promise<{ nuevos: number; eliminados: number }>,
        mensajeInicio: string,
    ) => {
        setEstado('sincronizando', mensajeInicio);
        setProgreso(0);

        try {
            const resultado = await fnSync((p: ProgresoSync) => {
                const porcentaje = p.total > 0 ? Math.round((p.actual / p.total) * 100) : 0;
                setProgreso(porcentaje);

                if (p.estado === 'descargando') {
                    agregarArchivo({
                        sampleId: p.sampleId,
                        nombre: p.nombre,
                        ruta: '',
                        estado: 'descargando',
                        tamano: 0,
                        descargadoEn: 0,
                    });
                } else if (p.estado === 'descargado') {
                    agregarArchivo({
                        sampleId: p.sampleId,
                        nombre: p.nombre,
                        ruta: p.ruta ?? '',
                        estado: 'descargado',
                        tamano: p.tamano ?? 0,
                        descargadoEn: Date.now(),
                    });
                } else if (p.estado === 'error') {
                    actualizarArchivoEstado(p.sampleId, 'error');
                }
            });

            setProgreso(100);
            setUltimaSync(Date.now());
            return resultado;
        } catch (err) {
            log.error('ejecutarSyncConProgreso fallo:', err);
            setEstado('error', 'Error durante la sincronización');
            setProgreso(0);
            return { nuevos: 0, eliminados: 0 };
        }
    }, [setEstado, setProgreso, agregarArchivo, actualizarArchivoEstado, setUltimaSync]);

    /* [2003A-38] Toggle de sincronización automática.
     * Al activar: trigger sync inmediata para que no quede esperando el poll.
     * Va debajo de ejecutarSyncConProgreso para evitar referencia antes de declaración. */
    const alternarSincronizacion = useCallback(async () => {
        const srv = obtenerSync();
        if (!srv) return;
        try {
            const nuevoEstado = !sincronizacionActiva;
            await srv.toggleSincronizacion(nuevoEstado);
            setActiva(nuevoEstado);

            if (nuevoEstado) {
                /* Activar → iniciar sync inmediata si hay carpeta */
                setEstado('sincronizando', 'Iniciando sincronización...');
                if (carpetaLocal) {
                    try {
                        const resultado = await ejecutarSyncConProgreso(
                            (onProgreso) => srv.sincronizarConServidor(onProgreso, { forzar: true }),
                            'Sincronizando...',
                        );
                        setEstado(
                            'completado',
                            `Sync completa: ${resultado.nuevos} nuevos, ${resultado.eliminados} eliminados`,
                        );
                        if (srv.obtenerHistorialSync) setHistorial(srv.obtenerHistorialSync(50));
                        if (srv.obtenerHistorialSamplesSync) setHistorialSamples(srv.obtenerHistorialSamplesSync(50));
                    } catch {
                        setEstado('error', 'Error al sincronizar');
                    }
                }
            } else {
                setEstado('pausado', 'Sincronización pausada');
            }
        } catch {
            setEstado('error', 'Error al cambiar sincronización');
        }
    }, [sincronizacionActiva, carpetaLocal, setActiva, setEstado, ejecutarSyncConProgreso, setHistorial, setHistorialSamples]);

    /* Sincronización manual inmediata con progreso en tiempo real.
     * Usa forzar:true para que funcione incluso con auto-sync pausado. */
    const sincronizarAhora = useCallback(async () => {
        if (!carpetaLocal) return;
        const srv = obtenerSync();
        if (!srv) return;
        try {
            const resultado = await ejecutarSyncConProgreso(
                (onProgreso) => srv.sincronizarConServidor(onProgreso, { forzar: true }),
                'Sincronizando...',
            );
            setEstado(
                'completado',
                `Sync completa: ${resultado.nuevos} nuevos, ${resultado.eliminados} eliminados`,
            );
            if (srv.obtenerHistorialSync) {
                setHistorial(srv.obtenerHistorialSync(50));
            }
            if (srv.obtenerHistorialSamplesSync) {
                setHistorialSamples(srv.obtenerHistorialSamplesSync(50));
            }
        } catch {
            setEstado('error', 'Error al sincronizar');
        }
    }, [carpetaLocal, ejecutarSyncConProgreso, setEstado, setHistorial, setHistorialSamples]);

    /* C358: Re-sync forzada (resetea tracking y re-descarga todo) */
    const forzarResyncAhora = useCallback(async () => {
        const srv = obtenerSync();
        if (!srv?.forzarResync || !carpetaLocal) return;
        try {
            const resultado = await ejecutarSyncConProgreso(
                (onProgreso) => srv.forzarResync!(onProgreso),
                'Re-sincronizando todo...',
            );
            setEstado(
                'completado',
                `Re-sync completa: ${resultado.nuevos} archivos descargados`,
            );
            /* Refrescar historial */
            if (srv.obtenerHistorialSync) {
                setHistorial(srv.obtenerHistorialSync(50));
            }
            if (srv.obtenerHistorialSamplesSync) {
                setHistorialSamples(srv.obtenerHistorialSamplesSync(50));
            }
        } catch {
            setEstado('error', 'Error al forzar re-sync');
        }
    }, [carpetaLocal, ejecutarSyncConProgreso, setEstado, setHistorial, setHistorialSamples]);

    /* Reforzar sync: reactiva samples borrados localmente y los re-descarga.
     * Menos agresivo que forzarResync (que resetea todo el tracking). */
    const reforzarSyncAhora = useCallback(async () => {
        const srv = obtenerSync();
        if (!srv?.reforzarSync || !carpetaLocal) return;
        try {
            const resultado = await ejecutarSyncConProgreso(
                (onProgreso) => srv.reforzarSync!(onProgreso),
                'Reforzando sincronización...',
            );
            setEstado(
                'completado',
                `Reforzar sync completa: ${resultado.nuevos} archivos recuperados`,
            );
            if (srv.obtenerHistorialSync) {
                setHistorial(srv.obtenerHistorialSync(50));
            }
            if (srv.obtenerHistorialSamplesSync) {
                setHistorialSamples(srv.obtenerHistorialSamplesSync(50));
            }
        } catch {
            setEstado('error', 'Error al reforzar sincronización');
        }
    }, [carpetaLocal, ejecutarSyncConProgreso, setEstado, setHistorial, setHistorialSamples]);

    return {
        alternarSincronizacion,
        sincronizarAhora,
        forzarResyncAhora,
        reforzarSyncAhora,
    };
}
