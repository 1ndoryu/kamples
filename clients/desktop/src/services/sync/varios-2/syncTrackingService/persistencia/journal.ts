import {
    appendOperacion,
    type OperacionJournal,
    type TipoOperacionJournal,
} from '../../../varios-1/syncJournal';
import { enLote, journalActivo } from '../estado';
import type {
    AccionHistorial,
    ArchivoTracking,
    BaseSyncLocal,
    ColeccionLocal,
    EntradaHistorialSample,
} from '../tipos';

/**
 * Aplicador de operaciones del journal. Usado durante recuperación (startup)
 * para re-aplicar operaciones pendientes sobre el último checkpoint.
 * Cada operación modifica el estado mutándolo in-place.
 */
export function aplicadorRecuperacion(op: OperacionJournal, estado: unknown): unknown {
    const s = estado as BaseSyncLocal;
    const d = op.datos as Record<string, unknown>;

    switch (op.tipo) {
        case 'TRACK_FILE': {
            s.archivos[d.clave as string] = d.archivo as ArchivoTracking;
            break;
        }
        case 'UNTRACK_FILE': {
            delete s.archivos[d.clave as string];
            break;
        }
        case 'ADD_COLLECTION': {
            const col = d.coleccion as ColeccionLocal;
            s.colecciones[col.id] = col;
            break;
        }
        case 'RENAME_COLLECTION': {
            const col = s.colecciones[d.id as number];
            if (col) {
                col.nombre = d.nombre as string;
                col.carpetaLocal = d.carpetaLocal as string;
            }
            break;
        }
        case 'DELETE_COLLECTION': {
            delete s.colecciones[d.id as number];
            break;
        }
        case 'MARK_DISABLED': {
            const archivo = s.archivos[d.clave as string];
            if (archivo) archivo.syncDeshabilitado = true;
            break;
        }
        case 'MARK_ENABLED': {
            const sampleId = d.sampleId as number;
            for (const [clave, arch] of Object.entries(s.archivos)) {
                if (arch.sampleId === sampleId) delete s.archivos[clave];
            }
            break;
        }
        case 'MOVE_FILE': {
            if (d.accion === 'agregarSinColeccion') {
                const sid = d.sampleId as number;
                if (!s.sinColeccion.includes(sid)) s.sinColeccion.push(sid);
            } else if (d.accion === 'quitarSinColeccion') {
                s.sinColeccion = s.sinColeccion.filter(id => id !== (d.sampleId as number));
            }
            break;
        }
        case 'UPDATE_HISTORIAL': {
            if (d.historialSamples !== undefined) {
                s.historialSamples = d.historialSamples as EntradaHistorialSample[];
            }
            if (d.historial !== undefined) {
                s.historial = d.historial as AccionHistorial[];
            }
            break;
        }
        case 'UPDATE_FILE': {
            /* Operación genérica — reservada para extensiones futuras */
            break;
        }
    }

    return s;
}

/**
 * Registra una operación en el journal WAL (solo escritura, sin re-aplicar).
 * Durante modo lote se omite — el checkpoint se hace en finalizarLote().
 */
export async function registrarEnJournal(tipo: TipoOperacionJournal, datosOp: unknown): Promise<void> {
    if (enLote || !journalActivo) return;
    try {
        await appendOperacion({ tipo, datos: datosOp }, true);
    } catch {
        /* Error de journal no bloquea la operación — datos en memoria se persisten en próximo checkpoint */
    }
}
