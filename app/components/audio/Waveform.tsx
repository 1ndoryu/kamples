'use client';

import {useRef, useEffect, useState, useCallback} from 'react';
import WaveSurfer from 'wavesurfer.js';
import styles from './Waveform.module.css';
import {useAudioPlayerStore} from '@/hooks/useAudioPlayer';

interface Props {
    idSample: number;
    urlAudio: string;
}

const opcionesWaveform = (container: HTMLElement) => ({
    container,
    waveColor: 'rgba(132, 132, 132, 0.5)',
    progressColor: '#d43333',
    height: 40,
    barWidth: 2.5,
    barGap: 1.5,
    barRadius: 0,
    cursorWidth: 0,
    interact: true,
    autoScroll: false
});

export default function Waveform({idSample, urlAudio}: Props) {
    const contenedorRef = useRef<HTMLDivElement>(null);
    const wavesurferRef = useRef<WaveSurfer | null>(null);
    const [cargandoOnda, setCargandoOnda] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const {alternarReproduccion, detenerPorCompleto, idSampleActual, instanciaActual, transicionando} =
        useAudioPlayerStore();

    const esEsteSampleElActual = idSampleActual === idSample;
    const estaSonando = esEsteSampleElActual && instanciaActual?.isPlaying();
    const estaEnTransicion = esEsteSampleElActual && transicionando;

    const inicializarWaveSurfer = useCallback(async () => {
        if (!contenedorRef.current || !urlAudio || wavesurferRef.current) return;

        try {
            const ws = WaveSurfer.create(opcionesWaveform(contenedorRef.current));
            wavesurferRef.current = ws;

            ws.on('finish', detenerPorCompleto);
            ws.on('error', err => {
                console.error('WaveSurfer runtime error:', err);
                setError(`Error: ${err.message}`);
            });
            ws.on('ready', () => setCargandoOnda(false));

            await ws.load(urlAudio);
        } catch (err) {
            const mensajeError = err instanceof Error ? err.message : 'Ocurrió un error al cargar el audio.';
            console.error('Error durante la inicialización de WaveSurfer:', err);
            setError(mensajeError);
            setCargandoOnda(false);
        }
    }, [urlAudio, detenerPorCompleto]);

    useEffect(() => {
        const observer = new IntersectionObserver(
            entries => {
                if (entries[0].isIntersecting && !wavesurferRef.current) {
                    inicializarWaveSurfer();
                    observer.disconnect();
                }
            },
            {threshold: 0.1}
        );

        if (contenedorRef.current) observer.observe(contenedorRef.current);

        return () => {
            observer.disconnect();
            wavesurferRef.current?.destroy();
            wavesurferRef.current = null;
        };
    }, [inicializarWaveSurfer]);

    const manejarClick = () => {
        if (!wavesurferRef.current || error || cargandoOnda || transicionando) return;
        alternarReproduccion(wavesurferRef.current, idSample);
    };

    return (
        <div className={styles.contenedorWaveform} onClick={manejarClick}>
            <button className={styles.botonPlayPause} disabled={cargandoOnda || estaEnTransicion}>
                {cargandoOnda || estaEnTransicion ? <div className={styles.loader}></div> : estaSonando ? '❚❚' : '▶'}
            </button>
            <div className={styles.waveArea}>
                {cargandoOnda && !error && <div className={styles.estadoCarga}>Cargando...</div>}
                {error && <div className={styles.estadoError}>{error}</div>}
                <div ref={contenedorRef} className={styles.wave} style={{opacity: cargandoOnda ? 0 : 1}} />
            </div>
        </div>
    );
}