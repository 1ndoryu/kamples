import {
    datos,
    indiceNombre,
    indiceRuta,
    indiceSampleHistorial,
    setDatos,
    sinColeccionSet,
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
    indiceRuta.clear();
    indiceNombre.clear();
    sinColeccionSet.clear();
    indiceSampleHistorial.clear();
    await persistir();
}
