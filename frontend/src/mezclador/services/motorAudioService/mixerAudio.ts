/*
 * Mixer por inserts: cadena EQ + fader + panner + analyser por insert,
 * y analysers del master para metering.
 */

import { estado } from './estadoMotor';
import { obtenerContexto } from './contextoAudio';

/* Crear BiquadFilterNode para una banda de EQ */
function crearBandaEQNodo(
    ctx: BaseAudioContext,
    tipo: BiquadFilterType,
    frecuencia: number,
    ganancia: number
): BiquadFilterNode {
    const filtro = ctx.createBiquadFilter();
    filtro.type = tipo;
    filtro.frequency.value = frecuencia;
    filtro.gain.value = ganancia;
    filtro.Q.value = 1;
    return filtro;
}

/* Crear cadena de nodos para un insert del mixer */
export function crearInsertMixer(insertId: number): void {
    const ctx = obtenerContexto();

    const inputGain = ctx.createGain();
    const fader = ctx.createGain();
    fader.gain.value = 0.8;
    const panner = ctx.createStereoPanner();
    const analyser = ctx.createAnalyser();
    analyser.fftSize = 256;

    /* EQ: 3 bandas paramétric — lowshelf, peaking, highshelf */
    const eqBandas: BiquadFilterNode[] = [
        crearBandaEQNodo(ctx, 'lowshelf', 200, 0),
        crearBandaEQNodo(ctx, 'peaking', 1000, 0),
        crearBandaEQNodo(ctx, 'highshelf', 8000, 0),
    ];

    /* Chain: input → EQ0 → EQ1 → EQ2 → fader → panner → analyser */
    inputGain.connect(eqBandas[0]);
    eqBandas[0].connect(eqBandas[1]);
    eqBandas[1].connect(eqBandas[2]);
    eqBandas[2].connect(fader);
    fader.connect(panner);
    panner.connect(analyser);

    /* El destino depende de si es master o insert regular */
    if (insertId === 0) {
        /*
         * C314: Si iniciar() ya creó masterAnalyser + stereo split,
         * reconectar la cadena del mixer a través de ellos en vez de
         * crear nodos duplicados. El analyser local del insert queda
         * para metering propio del insert 0.
         */
        if (estado.masterAnalyser) {
            /* Desconectar masterGain→masterAnalyser viejo, reconectar
               masterGain→insert chain→masterAnalyser→destination */
            estado.masterGain?.disconnect();
            estado.masterGain?.connect(inputGain);
            analyser.connect(estado.masterAnalyser);
        } else {
            /* Fallback si iniciar() no corrió primero (no debería pasar) */
            analyser.connect(ctx.destination);
            estado.masterAnalyser = analyser;
            estado.splitterEstereo = ctx.createChannelSplitter(2);
            estado.analyserL = ctx.createAnalyser();
            estado.analyserL.fftSize = 256;
            estado.analyserR = ctx.createAnalyser();
            estado.analyserR.fftSize = 256;
            analyser.connect(estado.splitterEstereo);
            estado.splitterEstereo.connect(estado.analyserL, 0);
            estado.splitterEstereo.connect(estado.analyserR, 1);
        }
    } else {
        /* Insert → Master inputGain */
        const masterNodes = estado.mixerInserts.get(0);
        if (masterNodes) {
            analyser.connect(masterNodes.inputGain);
        } else {
            /* Si el master aún no existe, conectar temporalmente a destination */
            analyser.connect(ctx.destination);
        }
    }

    estado.mixerInserts.set(insertId, { inputGain, fader, panner, eqBandas, analyser });
}

/* Inicializar todos los inserts del mixer (Master + 16) */
export function inicializarMixer(): void {
    if (estado.mixerInserts.size > 0) return;
    /* Crear Master primero */
    crearInsertMixer(0);
    /* Luego los inserts 1-16 (se conectan al master) */
    for (let i = 1; i <= 16; i++) {
        crearInsertMixer(i);
    }
}

/* Actualizar parámetros de un insert desde el store */
export function actualizarInsertMixer(insertId: number, volumen: number, pan: number, silenciado: boolean): void {
    const nodos = estado.mixerInserts.get(insertId);
    if (!nodos) return;
    nodos.fader.gain.value = silenciado ? 0 : volumen;
    nodos.panner.pan.value = pan;
}

/* Actualizar una banda EQ de un insert */
export function actualizarEQInsert(insertId: number, bandaIdx: number, frecuencia: number, ganancia: number, q: number): void {
    const nodos = estado.mixerInserts.get(insertId);
    if (!nodos || !nodos.eqBandas[bandaIdx]) return;
    const banda = nodos.eqBandas[bandaIdx];
    banda.frequency.value = frecuencia;
    banda.gain.value = ganancia;
    banda.Q.value = q;
}

/* Obtener peak levels de un insert */
export function obtenerPeaks(insertId: number): { peakL: number; peakR: number } {
    const nodos = estado.mixerInserts.get(insertId);
    if (!nodos) return { peakL: 0, peakR: 0 };

    const data = new Float32Array(nodos.analyser.frequencyBinCount);
    nodos.analyser.getFloatTimeDomainData(data);

    let max = 0;
    for (let i = 0; i < data.length; i++) {
        const abs = Math.abs(data[i]);
        if (abs > max) max = abs;
    }
    /* Mono simplificado; stereo split requeriría ChannelSplitter */
    return { peakL: max, peakR: max };
}

/* Obtener el AnalyserNode del master para visualización */
export function obtenerMasterAnalyser(): AnalyserNode | null {
    return estado.masterAnalyser;
}

/* C306: Obtener analysers estéreo L/R */
export function obtenerAnalyserEstereo(): { izquierdo: AnalyserNode | null; derecho: AnalyserNode | null } {
    return { izquierdo: estado.analyserL, derecho: estado.analyserR };
}

/* Destruir todos los nodos del mixer */
export function destruirMixerInserts(): void {
    for (const nodos of estado.mixerInserts.values()) {
        try {
            nodos.inputGain.disconnect();
            nodos.fader.disconnect();
            nodos.panner.disconnect();
            nodos.analyser.disconnect();
            for (const banda of nodos.eqBandas) {
                banda.disconnect();
            }
        } catch { /* nodos ya desconectados */ }
    }
    estado.mixerInserts.clear();
    estado.masterAnalyser = null;
}

/* Verificar si el mixer está inicializado */
export function esMixerInicializado(): boolean {
    return estado.mixerInserts.size > 0;
}
