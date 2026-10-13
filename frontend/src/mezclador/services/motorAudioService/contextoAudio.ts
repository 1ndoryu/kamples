/*
 * Ciclo de vida del AudioContext y del grafo maestro (masterGain, analysers).
 * El contexto se crea en iniciar(), nunca al importar el módulo.
 */

import { CONSTANTES_MEZCLADOR } from '../../types/mezclador';
import { crearLogger } from '../../../legacy/services/varios-1/logger';
import { estado } from './estadoMotor';

const log = crearLogger('MotorAudio');

/* Inicializar AudioContext — requiere gesto de usuario */
export function iniciar(): AudioContext {
    if (estado.contexto && estado.iniciado) return estado.contexto;

    try {
        estado.contexto = new AudioContext({
            sampleRate: CONSTANTES_MEZCLADOR.SAMPLE_RATE,
        });
    } catch (err) {
        /* Fallback sin sampleRate fijo si el navegador no lo soporta */
        log.error('Error creando AudioContext con sampleRate, intentando sin opciones', err);
        estado.contexto = new AudioContext();
    }
    estado.masterGain = estado.contexto.createGain();

    /*
     * C314: Crear masterAnalyser + stereo split desde iniciar(),
     * no solo en inicializarMixer(). El monitor de onda y peak meter
     * necesitan estos nodos aunque el mixer no esté inicializado.
     */
    estado.masterAnalyser = estado.contexto.createAnalyser();
    estado.masterAnalyser.fftSize = 2048;
    estado.masterGain.connect(estado.masterAnalyser);
    estado.masterAnalyser.connect(estado.contexto.destination);

    /* Stereo split para peak meters L/R */
    estado.splitterEstereo = estado.contexto.createChannelSplitter(2);
    estado.analyserL = estado.contexto.createAnalyser();
    estado.analyserL.fftSize = 256;
    estado.analyserR = estado.contexto.createAnalyser();
    estado.analyserR.fftSize = 256;
    estado.masterAnalyser.connect(estado.splitterEstereo);
    estado.splitterEstereo.connect(estado.analyserL, 0);
    estado.splitterEstereo.connect(estado.analyserR, 1);

    estado.iniciado = true;

    return estado.contexto;
}

/* Obtener o crear el contexto — reinicia si fue cerrado */
export function obtenerContexto(): AudioContext {
    if (!estado.contexto || !estado.iniciado || estado.contexto.state === 'closed') {
        return iniciar();
    }
    if (estado.contexto.state === 'suspended') {
        void estado.contexto.resume();
    }
    return estado.contexto;
}

/* Crear o reutilizar GainNode para una pista */
export function obtenerGainPista(pistaId: string): GainNode {
    const existente = estado.gainsCanales.get(pistaId);
    if (existente) return existente;

    const ctx = obtenerContexto();
    const gain = ctx.createGain();
    if (!estado.masterGain) {
        throw new Error('[MotorAudio] masterGain no inicializado');
    }
    gain.connect(estado.masterGain);
    estado.gainsCanales.set(pistaId, gain);
    return gain;
}

/* Obtener tiempo actual del contexto */
export function obtenerTiempoActual(): number {
    return estado.contexto?.currentTime ?? 0;
}

/* Desconectar y olvidar los gains por pista (usado al destruir) */
export function liberarGainsPistas(): void {
    for (const gain of estado.gainsCanales.values()) {
        try { gain.disconnect(); } catch { /* ya desconectado */ }
    }
    estado.gainsCanales.clear();
}

/*
 * Cerrar el AudioContext y resetear el estado de contexto.
 * Nulos tras el await: igual que antes de la partición.
 */
export async function cerrarContexto(): Promise<void> {
    if (estado.contexto && estado.contexto.state !== 'closed') {
        await estado.contexto.close();
    }
    estado.contexto = null;
    estado.masterGain = null;
    estado.masterAnalyser = null;
    estado.splitterEstereo = null;
    estado.analyserL = null;
    estado.analyserR = null;
    estado.iniciado = false;
}
