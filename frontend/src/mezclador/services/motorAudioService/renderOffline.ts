/*
 * Render offline de la mezcla para exportar (OfflineAudioContext).
 */

import { CONSTANTES_MEZCLADOR } from '../../types/mezclador';
import { crearLogger } from '../../../legacy/services/varios-1/logger';
import { obtenerBufferProcesado, obtenerBufferInvertido } from '../pitchShiftService';

const log = crearLogger('MotorAudio');

/* Crear OfflineAudioContext para exportar */
export function crearContextoOffline(duracion: number): OfflineAudioContext {
    const duracionSegura = Math.max(0.1, duracion);
    const totalFrames = Math.ceil(duracionSegura * CONSTANTES_MEZCLADOR.SAMPLE_RATE);
    try {
        return new OfflineAudioContext(
            CONSTANTES_MEZCLADOR.CANALES,
            totalFrames,
            CONSTANTES_MEZCLADOR.SAMPLE_RATE
        );
    } catch (err) {
        log.error('Error creando OfflineAudioContext', err);
        throw err;
    }
}

/* Renderizar mezcla offline */
export async function renderizarOffline(
    bloques: Array<{
        buffer: AudioBuffer;
        cuando: number;
        offset: number;
        duracion: number;
        playbackRate: number;
        volumen: number;
        invertido?: boolean;
        fadeIn?: number;
        fadeOut?: number;
        /* C240: Tonalidad en semitonos */
        detune?: number;
        /* C271: Modo tonal */
        modoTonalidad?: 'resample' | 'stretch';
        bloqueId?: string;
    }>,
    duracionTotal: number
): Promise<AudioBuffer> {
    const offlineCtx = crearContextoOffline(duracionTotal);
    const masterGain = offlineCtx.createGain();
    masterGain.connect(offlineCtx.destination);

    for (const bloque of bloques) {
        const fuente = offlineCtx.createBufferSource();
        const detuneVal = bloque.detune ?? 0;
        const modo = bloque.modoTonalidad ?? 'resample';

        /*
         * C271: En modo stretch, pre-procesar buffer con SoundTouch.
         * rate=1 y detune=0 para la fuente — ya aplicados por DSP.
         */
        let bufferParaFuente: AudioBuffer;
        let rateParaFuente: number;
        let detuneParaFuente: number;

        if (modo === 'stretch' && (detuneVal !== 0 || bloque.playbackRate !== 1)) {
            bufferParaFuente = obtenerBufferProcesado(
                offlineCtx,
                bloque.bloqueId ?? '',
                bloque.buffer,
                detuneVal,
                bloque.playbackRate
            );
            rateParaFuente = 1;
            detuneParaFuente = 0;
        } else {
            bufferParaFuente = bloque.buffer;
            rateParaFuente = bloque.playbackRate;
            detuneParaFuente = detuneVal;
        }

        /* C215: Invertir buffer si es necesario (cacheado) */
        if (bloque.invertido) {
            fuente.buffer = obtenerBufferInvertido(
                offlineCtx, bloque.bloqueId ?? '', bufferParaFuente
            );
        } else {
            fuente.buffer = bufferParaFuente;
        }

        fuente.playbackRate.value = rateParaFuente;

        if (detuneParaFuente !== 0) {
            fuente.detune.value = detuneParaFuente * 100;
        }

        const gainNodo = offlineCtx.createGain();
        fuente.connect(gainNodo);
        gainNodo.connect(masterGain);

        /* C215: Aplicar fades */
        const fadeIn = bloque.fadeIn ?? 0;
        const fadeOut = bloque.fadeOut ?? 0;

        if (fadeIn > 0 && fadeIn < bloque.duracion) {
            gainNodo.gain.setValueAtTime(0, bloque.cuando);
            gainNodo.gain.linearRampToValueAtTime(bloque.volumen, bloque.cuando + fadeIn);
        } else {
            gainNodo.gain.setValueAtTime(bloque.volumen, bloque.cuando);
        }

        if (fadeOut > 0 && fadeOut < bloque.duracion) {
            const inicioFade = bloque.cuando + bloque.duracion - fadeOut;
            if (inicioFade > bloque.cuando + fadeIn) {
                gainNodo.gain.setValueAtTime(bloque.volumen, inicioFade);
                gainNodo.gain.linearRampToValueAtTime(0, bloque.cuando + bloque.duracion);
            }
        }

        const offset = bloque.invertido
            ? Math.max(0, bufferParaFuente.duration - bloque.offset - (bloque.duracion * rateParaFuente))
            : bloque.offset;

        const tasaEfectivaOffline = rateParaFuente * Math.pow(2, (detuneParaFuente * 100) / 1200);
        fuente.start(bloque.cuando, offset, bloque.duracion * tasaEfectivaOffline);
    }

    return offlineCtx.startRendering();
}
