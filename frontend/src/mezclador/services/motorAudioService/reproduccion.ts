/*
 * Reproducción de buffers: programación de fuentes, canales del Channel Rack
 * hacia el mixer, step sequencer y control de volúmenes por pista.
 */

import { DECLIC_DURACIONES } from '../../types/mezclador';
import type { Patron } from '../../types/mezclador';
import { obtenerBufferProcesado, obtenerBufferInvertido } from '../pitchShiftService';
import { obtenerContexto, obtenerGainPista } from './contextoAudio';
import { estado } from './estadoMotor';

/*
 * Programar un buffer para reproducirse en un momento específico.
 * C215: Soporta invertido (reverse), fadeIn y fadeOut via GainNode ramps.
 * C271: Soporta modo stretch (pitch independiente de velocidad).
 * C287: Soporta pan estéreo y declicking automático.
 */
export function programarReproduccion(
    buffer: AudioBuffer,
    pistaId: string,
    cuando: number,
    offset: number,
    duracion: number,
    playbackRate: number,
    volumen: number,
    invertido = false,
    fadeIn = 0,
    fadeOut = 0,
    /* C240: Desplazamiento en semitonos (-12 a +12) */
    detune = 0,
    /* C271: Modo tonal — resample (vinilo) o stretch (SoundTouch) */
    modoTonalidad: 'resample' | 'stretch' = 'resample',
    /* C271: ID del bloque para cache SoundTouch */
    bloqueId = '',
    /* C287: Balance estéreo (-1 a 1) */
    pan = 0,
    /* C287: Modo declicking (micro-fade anti-click) */
    modoDeclic: 'none' | 'corto' | 'medio' | 'largo' = 'none'
): AudioBufferSourceNode {
    const ctx = obtenerContexto();
    const fuente = ctx.createBufferSource();

    /*
     * C271: En modo stretch, pre-procesar el buffer con SoundTouch.
     * El pitch se controla via DSP, no via detune nativo.
     * playbackRate se mantiene en 1 porque SoundTouch ya lo aplica.
     */
    let bufferFinal: AudioBuffer;
    let rateParaFuente: number;
    let detuneParaFuente: number;

    if (modoTonalidad === 'stretch' && (detune !== 0 || playbackRate !== 1)) {
        /* SoundTouch procesa tanto pitch como tempo */
        bufferFinal = obtenerBufferProcesado(
            ctx, bloqueId, buffer, detune, playbackRate
        );
        rateParaFuente = 1;
        detuneParaFuente = 0;
    } else {
        bufferFinal = buffer;
        rateParaFuente = playbackRate;
        detuneParaFuente = detune;
    }

    /*
     * C215: Si el bloque está invertido, usar buffer invertido cacheado.
     */
    if (invertido) {
        const bufferInvertido = obtenerBufferInvertido(ctx, bloqueId, bufferFinal);
        fuente.buffer = bufferInvertido;
        offset = Math.max(0, bufferFinal.duration - offset - (duracion * rateParaFuente));
    } else {
        fuente.buffer = bufferFinal;
    }

    fuente.playbackRate.value = rateParaFuente;

    /*
     * C258 fix: Detune puro sin compensación de playbackRate.
     * Solo aplica en modo resample (vinilo).
     * En modo stretch, detune ya fue procesado por SoundTouch.
     */
    if (detuneParaFuente !== 0) {
        fuente.detune.value = detuneParaFuente * 100;
    }

    const gainNodo = ctx.createGain();
    fuente.connect(gainNodo);

    /*
     * C287: StereoPannerNode para balance L/R.
     * pan=0 → centro, -1 → izquierda, 1 → derecha.
     */
    const gainPista = obtenerGainPista(pistaId);
    if (pan !== 0) {
        const panNodo = ctx.createStereoPanner();
        panNodo.pan.value = Math.max(-1, Math.min(1, pan));
        gainNodo.connect(panNodo);
        panNodo.connect(gainPista);
    } else {
        gainNodo.connect(gainPista);
    }

    /*
     * C287: Declicking — micro-fade al inicio/fin para evitar clicks digitales.
     * Se aplica ADEMÁS de los fades visibles del usuario.
     */
    const declicDur = DECLIC_DURACIONES[modoDeclic] ?? 0;

    /*
     * C215: Aplicar fade in/out vía rampas de ganancia.
     * C287: Si hay declicking, el fade mínimo es el declicking duration.
     */
    const fadeInEfectivo = Math.max(fadeIn, declicDur);
    if (fadeInEfectivo > 0 && fadeInEfectivo < duracion) {
        gainNodo.gain.setValueAtTime(0, cuando);
        gainNodo.gain.linearRampToValueAtTime(volumen, cuando + fadeInEfectivo);
    } else {
        gainNodo.gain.setValueAtTime(volumen, cuando);
    }

    const fadeOutEfectivo = Math.max(fadeOut, declicDur);
    if (fadeOutEfectivo > 0 && fadeOutEfectivo < duracion) {
        const inicioFadeOut = cuando + duracion - fadeOutEfectivo;
        if (inicioFadeOut > cuando + fadeInEfectivo) {
            gainNodo.gain.setValueAtTime(volumen, inicioFadeOut);
            gainNodo.gain.linearRampToValueAtTime(0, cuando + duracion);
        }
    }

    /*
     * C222+C258: duration en buffer-time.
     * En modo stretch, rate=1 y detune=0 → tasaEfectiva=1 (duración no cambia).
     * En modo resample, la tasa efectiva incluye detune nativo.
     */
    const tasaEfectiva = rateParaFuente * Math.pow(2, (detuneParaFuente * 100) / 1200);
    fuente.start(cuando, offset, duracion * tasaEfectiva);
    estado.nodosActivos.push(fuente);

    fuente.onended = () => {
        gainNodo.disconnect();
        estado.nodosActivos = estado.nodosActivos.filter(n => n !== fuente);
    };

    return fuente;
}

/* Detener toda la reproducción */
export function detenerTodo(): void {
    for (const nodo of estado.nodosActivos) {
        try { nodo.stop(); } catch { /* ya terminó */ }
    }
    estado.nodosActivos = [];
}

/* Ajustar volumen master */
export function setVolumenMaster(volumen: number): void {
    if (estado.masterGain) {
        estado.masterGain.gain.value = volumen;
    }
}

/* Ajustar volumen de una pista */
export function setVolumenPista(pistaId: string, volumen: number): void {
    const gain = estado.gainsCanales.get(pistaId);
    if (gain) gain.gain.value = volumen;
}

/* Silenciar/des-silenciar pista. volumenReal restaura el gain correcto al des-silenciar */
export function setSilenciarPista(pistaId: string, silenciada: boolean, volumenReal = 1): void {
    const gain = estado.gainsCanales.get(pistaId);
    if (gain) gain.gain.value = silenciada ? 0 : volumenReal;
}

/* Programar reproducción de un canal del Channel Rack a un insert del mixer */
export function programarReproduccionCanal(
    buffer: AudioBuffer,
    canalId: string,
    mixerInsertId: number,
    cuando: number,
    offset: number,
    duracion: number,
    playbackRate: number,
    volumen: number,
    invertido = false,
    fadeIn = 0,
    fadeOut = 0,
    detune = 0,
    modoTonalidad: 'resample' | 'stretch' = 'resample',
    bloqueId = '',
    pan = 0,
    modoDeclic: 'none' | 'corto' | 'medio' | 'largo' = 'corto'
): AudioBufferSourceNode | null {
    const ctx = obtenerContexto();

    /* Verificar que el insert del mixer existe */
    const insertNodes = estado.mixerInserts.get(mixerInsertId);
    if (!insertNodes) {
        /* Fallback: reproducir por el sistema legacy */
        return programarReproduccion(
            buffer, canalId, cuando, offset, duracion,
            playbackRate, volumen, invertido, fadeIn, fadeOut,
            detune, modoTonalidad, bloqueId, pan, modoDeclic
        );
    }

    const fuente = ctx.createBufferSource();

    /* Procesamiento de buffer (stretch/invertido) */
    let bufferFinal: AudioBuffer;
    let rateParaFuente: number;
    let detuneParaFuente: number;

    if (modoTonalidad === 'stretch' && (detune !== 0 || playbackRate !== 1)) {
        bufferFinal = obtenerBufferProcesado(ctx, bloqueId, buffer, detune, playbackRate);
        rateParaFuente = 1;
        detuneParaFuente = 0;
    } else {
        bufferFinal = buffer;
        rateParaFuente = playbackRate;
        detuneParaFuente = detune;
    }

    if (invertido) {
        fuente.buffer = obtenerBufferInvertido(ctx, bloqueId, bufferFinal);
    } else {
        fuente.buffer = bufferFinal;
    }

    fuente.playbackRate.value = rateParaFuente;
    if (detuneParaFuente !== 0) {
        fuente.detune.value = detuneParaFuente * 100;
    }

    /* Gain del canal → conectar al insert del mixer */
    const gainNodo = ctx.createGain();
    fuente.connect(gainNodo);

    /* Pan per-canal antes del mixer */
    if (pan !== 0) {
        const panNodo = ctx.createStereoPanner();
        panNodo.pan.value = Math.max(-1, Math.min(1, pan));
        gainNodo.connect(panNodo);
        panNodo.connect(insertNodes.inputGain);
    } else {
        gainNodo.connect(insertNodes.inputGain);
    }

    /* Declicking + fades */
    const declicDur = DECLIC_DURACIONES[modoDeclic] ?? 0;
    const fadeInEfectivo = Math.max(fadeIn, declicDur);
    if (fadeInEfectivo > 0 && fadeInEfectivo < duracion) {
        gainNodo.gain.setValueAtTime(0, cuando);
        gainNodo.gain.linearRampToValueAtTime(volumen, cuando + fadeInEfectivo);
    } else {
        gainNodo.gain.setValueAtTime(volumen, cuando);
    }

    const fadeOutEfectivo = Math.max(fadeOut, declicDur);
    if (fadeOutEfectivo > 0 && fadeOutEfectivo < duracion) {
        const inicioFadeOut = cuando + duracion - fadeOutEfectivo;
        if (inicioFadeOut > cuando + fadeInEfectivo) {
            gainNodo.gain.setValueAtTime(volumen, inicioFadeOut);
            gainNodo.gain.linearRampToValueAtTime(0, cuando + duracion);
        }
    }

    const tasaEfectiva = rateParaFuente * Math.pow(2, (detuneParaFuente * 100) / 1200);
    fuente.start(cuando, offset, duracion * tasaEfectiva);
    estado.nodosActivos.push(fuente);

    fuente.onended = () => {
        gainNodo.disconnect();
        estado.nodosActivos = estado.nodosActivos.filter(n => n !== fuente);
    };

    return fuente;
}

/* STEP SEQUENCER — Reproducción de patrones */

/*
 * Programar un patrón completo para reproducción.
 * Cada paso activo produce un trigger de sample.
 * Se usa tanto en modo PAT como en SONG (con offset).
 */
export function programarPatron(
    patron: Patron,
    bpm: number,
    desdeSegundo: number,
    mixerInicializado: boolean
): void {
    /* Duración de un paso = 1 semicorchea = (60/bpm) / 4 */
    const duracionPasoReal = (60 / bpm) / 4;

    for (const canal of patron.canales) {
        if (canal.silenciado || !canal.audioBuffer) continue;

        /* Si hay algún canal en solo, solo reproducir esos */
        const haySolo = patron.canales.some(c => c.solo);
        if (haySolo && !canal.solo) continue;

        for (let i = 0; i < canal.pasos.length; i++) {
            const paso = canal.pasos[i];
            if (!paso.activo) continue;

            /* Momento en que suena este paso */
            let cuando = desdeSegundo + (i * duracionPasoReal);

            /* Swing: desplazar pasos impares */
            if (patron.swing > 0 && i % 2 === 1) {
                cuando += duracionPasoReal * patron.swing * 0.5;
            }

            const ctx = obtenerContexto();
            const ahora = ctx.currentTime;

            /* Solo programar pasos que están en el futuro */
            if (cuando < ahora - 0.01) continue;

            const volumenFinal = paso.velocity * canal.volumen;
            const duracionSample = canal.audioBuffer.duration;

            if (mixerInicializado) {
                programarReproduccionCanal(
                    canal.audioBuffer,
                    canal.id,
                    canal.mixerInsertId,
                    cuando,
                    0,
                    duracionSample,
                    1,
                    volumenFinal,
                    false,
                    0, 0,
                    paso.pitch,
                    'resample',
                    canal.id,
                    paso.pan !== 0 ? paso.pan : canal.pan,
                    'corto'
                );
            } else {
                programarReproduccion(
                    canal.audioBuffer,
                    canal.id,
                    cuando,
                    0,
                    duracionSample,
                    1,
                    volumenFinal,
                    false,
                    0, 0,
                    paso.pitch,
                    'resample',
                    canal.id,
                    paso.pan !== 0 ? paso.pan : canal.pan,
                    'corto'
                );
            }
        }
    }
}
