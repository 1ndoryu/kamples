'use client';

import {useRef, useEffect, useState, useCallback} from 'react';
import WaveSurfer from 'wavesurfer.js';
import styles from './Waveform.module.css';
import {useAudioPlayerStore} from '@/hooks/useAudioPlayer';

interface Props {
    idSample: number;
    urlAudio: string;
    /** Si es true, solo renderiza el botón play/pause sin mostrar la forma de onda */
    soloBoton?: boolean;
    /** Si es true, oculta el botón (útil cuando ya hay uno superpuesto) */
    ocultarBoton?: boolean;
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

export default function Waveform({idSample, urlAudio, soloBoton = false, ocultarBoton = false}: Props) {
    const contenedorRef = useRef<HTMLDivElement>(null);
    const wavesurferRef = useRef<WaveSurfer | null>(null);
    const [cargandoOnda, setCargandoOnda] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const {alternarReproduccion, detenerPorCompleto, idSampleActual, instanciaActual, transicionando} = useAudioPlayerStore();

    const esEsteSampleElActual = idSampleActual === idSample;
    const estaSonando = esEsteSampleElActual && instanciaActual?.isPlaying();
    const estaEnTransicion = esEsteSampleElActual && transicionando;

    const inicializarWaveSurfer = useCallback(async () => {
        if (!contenedorRef.current || !urlAudio || wavesurferRef.current) return;

        try {
            const ws = WaveSurfer.create(opcionesWaveform(contenedorRef.current));
            wavesurferRef.current = ws;

            // Registrar esta instancia en el store para poder sincronizarla con otras representaciones del mismo sample
            useAudioPlayerStore.getState().registrarInstancia(idSample, ws);

            ws.on('finish', detenerPorCompleto);
            ws.on('error', err => {
                if (err instanceof Error && err.name === 'AbortError') {
                    // AbortError esperado cuando se destruye la instancia antes de terminar la descarga.
                    return;
                }

                const mensajeError = err instanceof Error ? err.message : 'Ocurrió un error al cargar el audio.';
                console.error('Error durante la inicialización de WaveSurfer:', err);
                setError(mensajeError);
                setCargandoOnda(false);
            });
            ws.on('ready', () => setCargandoOnda(false));

            await ws.load(urlAudio);
        } catch (err) {
            if (err instanceof Error && err.name === 'AbortError') {
                // AbortError esperado cuando se destruye la instancia antes de terminar la descarga.
                return;
            }

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
            if (wavesurferRef.current) {
                useAudioPlayerStore.getState().eliminarInstancia(idSample, wavesurferRef.current);
            }
            wavesurferRef.current = null;
        };
    }, [inicializarWaveSurfer]);

    const manejarClick = () => {
        if (!wavesurferRef.current || error || cargandoOnda || transicionando) return;
        alternarReproduccion(wavesurferRef.current, idSample);
    };

    // Si solo queremos el botón, inicializar WaveSurfer en cuanto sea posible
    useEffect(() => {
        if (soloBoton && contenedorRef.current && !wavesurferRef.current) {
            inicializarWaveSurfer();
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [soloBoton, contenedorRef.current]);

    const contenedorClase = soloBoton ? `${styles.contenedorWaveform} ${styles.soloBoton}` : styles.contenedorWaveform;
    const waveAreaStyle = soloBoton ? {display: 'none'} : undefined;

    return (
        <div className={contenedorClase} onClick={manejarClick}>
            {!ocultarBoton && (
                <button className={styles.botonPlayPause} disabled={cargandoOnda || estaEnTransicion}>
                    {cargandoOnda || estaEnTransicion ? <div className={styles.loader}></div> : estaSonando ? '❚❚' : '▶'}
                </button>
            )}
            <div className={styles.waveArea} style={waveAreaStyle}>
                {cargandoOnda && !error && <div className={styles.estadoCarga}>Cargando...</div>}
                {error && <div className={styles.estadoError}>{error}</div>}
                <div ref={contenedorRef} className={styles.wave} style={{opacity: cargandoOnda ? 0 : 1}} />
            </div>
        </div>
    );
}
