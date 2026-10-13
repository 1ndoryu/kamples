/*
 * Borrado local: papelera + soft-delete en servidor con rate-limit por ciclo (QL63).
 */
import { esMovimientoInterno, obtenerBaseUrlSync } from '../../varios-1/syncGuards';
import { estaOnline } from '../../../varios-1/desktopService';
import { encolarOperacion } from '../../../varios-1/offlineQueueService';
import { logSync } from '../../varios-1/syncLogger';
import { estado, buscarEnIndicePorRuta } from '../syncState';
import { marcarNoSincronizar } from './muestras';
import { borradosEnCiclo, ultimoResetBorrados, colaBorradosPendientes, setBorradosEnCiclo, setUltimoResetBorrados } from './syncWatcherEstado';

/* Borrado local → papelera + borrado en servidor si configurado */

/*
 * Maneja la eliminacion de un archivo local detectada por el watcher.
 * 1. Si la papelera esta activa, el archivo ya fue movido por el OS (watcher detecta remove).
 *    Registrar en tracking como no_sincronizar.
 * 2. Si borrado bidireccional (local→servidor) esta activo, llamar soft-delete en servidor.
 * 3. Rate-limit: maximo 50 borrados en servidor por ciclo de sync.
 */
const LIMITE_BORRADOS_POR_CICLO = 50;

export async function manejarBorradoLocal(ruta: string): Promise<void> {
    /*
     * Defensa: si esta ruta fue marcada como movimiento interno
     * (ej: rename de moverArchivoASinColeccion), ignorar el DELETE.
     * Sin esto, un fallo en la actualización de tracking podría causar
     * soft-delete del sample recién subido cuando borrarEnServidorAlBorrarLocal está activo.
     */
    if (esMovimientoInterno(ruta)) {
        logSync.info('syncWatcherSetup', '[Sync] Ignorando DELETE de ruta movida internamente:', ruta);
        return;
    }

    /* Marcar como no sincronizar (comportamiento base) */
    await marcarNoSincronizar(ruta);

    /* Verificar si borrado bidireccional local→servidor esta activo */
    if (!estado.configAvanzada.borrarEnServidorAlBorrarLocal) return;

    /* Rate-limit: resetear contador cada 5 minutos y drenar cola pendiente */
    const ahora = Date.now();
    if (ahora - ultimoResetBorrados > 5 * 60 * 1000) {
        setBorradosEnCiclo(0);
        setUltimoResetBorrados(ahora);
        /* QL63: Drenar cola de borrados pendientes del ciclo anterior */
        while (colaBorradosPendientes.length > 0 && borradosEnCiclo < LIMITE_BORRADOS_POR_CICLO) {
            const pendienteId = colaBorradosPendientes.shift()!;
            await softDeleteEnServidor(pendienteId);
            setBorradosEnCiclo(borradosEnCiclo + 1);
        }
    }

    if (borradosEnCiclo >= LIMITE_BORRADOS_POR_CICLO) {
        /* QL63: Encolar para siguiente ciclo en vez de ignorar silenciosamente */
        const rutaNorm = ruta.replace(/\\/g, '/');
        const archivo = buscarEnIndicePorRuta(rutaNorm);
        const sid = archivo?.sampleId ?? estado.trackingModule?.buscarArchivoPorRuta(ruta)?.sampleId;
        if (sid) {
            colaBorradosPendientes.push(sid);
            logSync.warn('syncWatcherSetup', `[Sync] Limite borrados alcanzado (${LIMITE_BORRADOS_POR_CICLO}/ciclo). Encolado para siguiente ciclo:`, sid);
        }
        return;
    }

    /* Buscar sampleId en indice */
    const rutaNorm = ruta.replace(/\\/g, '/');
    const archivo = buscarEnIndicePorRuta(rutaNorm);
    const sampleId = archivo?.sampleId;

    if (!sampleId) {
        /* Intentar buscar en tracking v2 */
        const archivoV2 = estado.trackingModule?.buscarArchivoPorRuta(ruta);
        if (!archivoV2) return;
        await softDeleteEnServidor(archivoV2.sampleId);
        setBorradosEnCiclo(borradosEnCiclo + 1);
        return;
    }

    await softDeleteEnServidor(sampleId);
    setBorradosEnCiclo(borradosEnCiclo + 1);
}

/*
 * Soft-delete de un sample en el servidor.
 * Marca como eliminado sin borrar permanentemente (el servidor puede tener su propia papelera).
 */
async function softDeleteEnServidor(sampleId: number): Promise<boolean> {
    if (!estaOnline()) {
        encolarOperacion({
            tipo: 'soft_delete',
            endpoint: `${obtenerBaseUrlSync()}/kamples/v1/samples/${sampleId}`,
            method: 'DELETE',
            body: { soft: true },
            claveDuplicacion: `soft_delete_${sampleId}`,
        });
        return true;
    }

    try {
        const baseUrl = obtenerBaseUrlSync();
        const resp = await fetch(`${baseUrl}/kamples/v1/samples/${sampleId}`, {
            method: 'DELETE',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ soft: true }),
        });

        if (!resp.ok) {
            logSync.error('syncWatcherSetup', '[Sync] Error en soft-delete servidor:', [sampleId, resp.status]);
            return false;
        }

        logSync.info('syncWatcherSetup', '[Sync] Sample eliminado en servidor (soft-delete):', sampleId);
        return true;
    } catch (err) {
        logSync.error('syncWatcherSetup', '[Sync] Error en soft-delete:', [sampleId, err]);
        return false;
    }
}
