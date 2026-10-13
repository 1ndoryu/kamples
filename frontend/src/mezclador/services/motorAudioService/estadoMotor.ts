/*
 * Estado compartido del motor de audio (singleton).
 * Hoja del grafo de módulos: solo datos, sin lógica ni efectos al importar.
 * Los módulos leen y escriben `estado.campo`. Las reasignaciones
 * (p. ej. `estado.nodosActivos = ...`) se hacen sobre el objeto, nunca sobre
 * una copia local, para que todos los módulos vean el mismo valor.
 */

import type { MixerInsertNodes } from '../../types/mezclador';

export interface EstadoMotor {
    contexto: AudioContext | null;
    masterGain: GainNode | null;
    cacheBuffers: Map<string, AudioBuffer>;
    nodosActivos: AudioBufferSourceNode[];
    gainsCanales: Map<string, GainNode>;
    iniciado: boolean;

    /* Mixer: nodos Web Audio por insert */
    mixerInserts: Map<number, MixerInsertNodes>;
    masterAnalyser: AnalyserNode | null;

    /* C306: Stereo split para peak meters L/R */
    splitterEstereo: ChannelSplitterNode | null;
    analyserL: AnalyserNode | null;
    analyserR: AnalyserNode | null;
}

export const estado: EstadoMotor = {
    contexto: null,
    masterGain: null,
    cacheBuffers: new Map<string, AudioBuffer>(),
    nodosActivos: [],
    gainsCanales: new Map<string, GainNode>(),
    iniciado: false,
    mixerInserts: new Map<number, MixerInsertNodes>(),
    masterAnalyser: null,
    splitterEstereo: null,
    analyserL: null,
    analyserR: null,
};
