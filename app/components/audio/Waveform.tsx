'use client';

import {useRef, useEffect, useState, useCallback} from 'react';
import WaveSurfer from 'wavesurfer.js';
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

    const {alternarReproduccion, detenerPorCompleto, idSampleActual, instanciaActual, transicionando} = useAudioPlayerStore();

    const esEsteSampleElActual = idSampleActual === idSample;
    const estaSonando = esEsteSampleElActual && instanciaActual?.isPlaying();
    const estaEnTransicion = esEsteSampleElActual && transicionando;

    const inicializarWaveSurfer = useCallback(async () => {
        if (!contenedorRef.current || !urlAudio || wavesurferRef.current) return;

        try {
            const secureUrlRes = await fetch(`/api/auth/audio/secure-url?mediaUrl=${encodeURIComponent(urlAudio)}`);
            if (!secureUrlRes.ok) throw new Error('No se pudo obtener la URL segura del audio.');
            const {secureUrl} = await secureUrlRes.json();

            const ws = WaveSurfer.create(opcionesWaveform(contenedorRef.current));
            wavesurferRef.current = ws;

            ws.on('finish', detenerPorCompleto);
            ws.on('error', err => {
                console.error('WaveSurfer runtime error:', err);
                setError(`Error: ${err.message}`);
            });
            ws.on('ready', () => setCargandoOnda(false));

            await ws.load(secureUrl);
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
        <div className="contenedorWaveform" onClick={manejarClick}>
            <button className="botonPlayPause" disabled={cargandoOnda || estaEnTransicion}>
                {cargandoOnda || estaEnTransicion ? <div className="loader"></div> : estaSonando ? '❚❚' : '▶'}
            </button>
            <div className="waveArea">
                {cargandoOnda && !error && <div className="estadoCarga">Cargando...</div>}
                {error && <div className="estadoError">{error}</div>}
                <div ref={contenedorRef} className="wave" style={{opacity: cargandoOnda ? 0 : 1}} />
            </div>
            <style jsx>{`
                .contenedorWaveform {
                    display: flex;
                    align-items: center;
                    gap: 8px;
                    flex-grow: 1;
                    cursor: pointer;
                    min-width: 0;
                    border-radius: var(--radius);
                    transition: background-color 0.2s;
                    padding-right: 8px;
                }
                .contenedorWaveform:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                }
                .waveArea {
                    flex-grow: 1;
                    align-items: center;
                    height: 40px;
                    display: flex;
                    position: relative;
                    max-width: 250px;
                    margin-left: auto;
                }
                .wave {
                    width: 100%;
                }
                .botonPlayPause {
                    background: transparent;
                    border: none;
                    color: var(--color-texto);
                    font-size: 1.2rem;
                    cursor: pointer;
                    width: 40px;
                    height: 40px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    flex-shrink: 0;
                    opacity: 0.8;
                    transition: opacity 0.2s;
                    padding: unset;
                }
                .botonPlayPause:disabled {
                    cursor: not-allowed;
                    opacity: 0.5;
                }
                .botonPlayPause:hover:not(:disabled) {
                    opacity: 1;
                }
                .estadoCarga,
                .estadoError {
                    font-size: 0.75rem;
                    opacity: 0.7;
                    color: var(--color-texto);
                    padding-left: 10px;
                    pointer-events: none;
                }
                .estadoError {
                    color: #e53e3e;
                }
                .loader {
                    border: 2px solid #f3f3f333;
                    border-top: 2px solid var(--color-texto);
                    border-radius: 50%;
                    width: 16px;
                    height: 16px;
                    animation: spin 1s linear infinite;
                }
                @keyframes spin {
                    0% {
                        transform: rotate(0deg);
                    }
                    100% {
                        transform: rotate(360deg);
                    }
                }
            `}</style>
        </div>
    );
}
