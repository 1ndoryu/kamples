import {
    datos,
    estadoSyncTracking,
    setDatos,
} from './estado';
import { persistir } from './persistencia/persistencia';

/* Reset completo */

export async function resetearTracking(): Promise<void> {
    const userIdActual = datos.userId;
    setDatos({
        archivos: {},
        colecciones: {},
        sinColeccion: [],
        historial: [],
        historialSamples: [],
        userId: userIdActual,
    });
    estadoSyncTracking.indiceRuta.clear();
    estadoSyncTracking.indiceNombre.clear();
    estadoSyncTracking.sinColeccionSet.clear();
    estadoSyncTracking.indiceSampleHistorial.clear();
    await persistir();
}
