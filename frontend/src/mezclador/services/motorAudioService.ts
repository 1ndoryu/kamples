/*
 * motorAudioService — Barril del motor de audio del mezclador.
 * Singleton AudioContext + caché de buffers + reproducción con scheduling
 * (lookahead) + render offline + mixer por inserts.
 * La implementación vive en ./motorAudioService/ y no importa de este archivo.
 */

import {
    iniciar,
    obtenerContexto,
    obtenerGainPista,
    obtenerTiempoActual,
    liberarGainsPistas,
    cerrarContexto,
} from './motorAudioService/contextoAudio';
import {
    cargarBuffer,
    obtenerBuffer,
    decodificarBufferLocal,
    limpiarCache,
} from './motorAudioService/buffersAudio';
import {
    programarReproduccion,
    detenerTodo,
    setVolumenMaster,
    setVolumenPista,
    setSilenciarPista,
    programarReproduccionCanal,
    programarPatron,
} from './motorAudioService/reproduccion';
import { crearContextoOffline, renderizarOffline } from './motorAudioService/renderOffline';
import {
    crearInsertMixer,
    inicializarMixer,
    actualizarInsertMixer,
    actualizarEQInsert,
    obtenerPeaks,
    obtenerMasterAnalyser,
    obtenerAnalyserEstereo,
    destruirMixerInserts,
    esMixerInicializado,
} from './motorAudioService/mixerAudio';

/* Limpiar por completo al cerrar el mezclador — libera AudioContext */
async function destruir(): Promise<void> {
    detenerTodo();
    liberarGainsPistas();
    /* Desconectar mixer inserts */
    destruirMixerInserts();
    await cerrarContexto();
    limpiarCache();
}

/* Singleton global */
export const motorAudio = {
    iniciar,
    obtenerContexto,
    cargarBuffer,
    obtenerBuffer,
    decodificarBufferLocal,
    obtenerGainPista,
    programarReproduccion,
    detenerTodo,
    setVolumenMaster,
    setVolumenPista,
    setSilenciarPista,
    obtenerTiempoActual,
    crearContextoOffline,
    renderizarOffline,
    destruir,
    crearInsertMixer,
    inicializarMixer,
    actualizarInsertMixer,
    actualizarEQInsert,
    obtenerPeaks,
    obtenerMasterAnalyser,
    obtenerAnalyserEstereo,
    programarReproduccionCanal,
    programarPatron,
    esMixerInicializado,
    limpiarCache,
};
