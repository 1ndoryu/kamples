// app/components/audio/Waveform.tsx
'use client';

import {useRef, useEffect, useState, useCallback} from 'react';
import WaveSurfer from 'wavesurfer.js';
import {useAudioPlayerStore} from '@/hooks/useAudioPlayer';

interface Props {
    idSample: number;
    urlAudio: string;
    onPlay: () => void;
    onPause: () => void;
}

const opcionesWaveform = (container: HTMLElement) => ({
    container,
    waveColor: 'rgba(132, 132, 132, 0.5)',
    progressColor: '#d43333',
    height: 40,
    barWidth: 2,
    barGap: 1.5,
    barRadius: 2,
    cursorWidth: 0,
    interact: true,
    autoScroll: false
});

export default function Waveform({idSample, urlAudio, onPlay, onPause}: Props) {
    const contenedorRef = useRef<HTMLDivElement>(null);
    const wavesurferRef = useRef<WaveSurfer | null>(null);
    const [cargando, setCargando] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const {reproducir, detener, idSampleActual} = useAudioPlayerStore();

    const inicializarWaveSurfer = useCallback(async () => {
        if (!contenedorRef.current || !urlAudio) return;

        // Obtener la URL segura del backend
        try {
            const res = await fetch(`/api/audio/secure-url?mediaUrl=${encodeURIComponent(urlAudio)}`);
            if (!res.ok) throw new Error('No se pudo obtener la URL segura.');
            const {secureUrl} = await res.json();

            const ws = WaveSurfer.create(opcionesWaveform(contenedorRef.current));
            wavesurferRef.current = ws;

            ws.on('ready', () => {
                setCargando(false);
            });

            ws.on('play', onPlay);

            ws.on('pause', onPause);

            ws.on('finish', () => {
                detener(); // Aseguramos que se marque como detenido globalmente
                onPause();
            });

            ws.on('error', err => {
                console.error('WaveSurfer error:', err);
                setError('Error al cargar audio.');
                setCargando(false);
            });

            // La carga se hace con el stream de la API segura
            ws.load(secureUrl);
        } catch (err) {
            console.error(err);
            setError('Error de red.');
            setCargando(false);
        }
    }, [urlAudio, onPlay, onPause, detener]);

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

        if (contenedorRef.current) {
            observer.observe(contenedorRef.current);
        }

        return () => {
            observer.disconnect();
            wavesurferRef.current?.destroy();
        };
    }, [inicializarWaveSurfer]);

    const togglePlay = () => {
        if (!wavesurferRef.current) return;
        if (wavesurferRef.current.isPlaying()) {
            detener();
        } else {
            reproducir(wavesurferRef.current, idSample);
        }
    };

    // Detener si otro sample empieza a sonar
    useEffect(() => {
        if (idSampleActual !== idSample && wavesurferRef.current?.isPlaying()) {
            wavesurferRef.current.pause();
        }
    }, [idSampleActual, idSample]);

    return (
        <div className="waveformContenedor" onClick={togglePlay}>
            {cargando && !error && <div className="estadoCarga">Cargando onda...</div>}
            {error && <div className="estadoError">{error}</div>}
            <div ref={contenedorRef} className="wave"></div>
            <style jsx>{`
                .waveformContenedor {
                    flex-grow: 1;
                    cursor: pointer;
                    position: relative;
                    min-height: 40px; /* Altura fija para evitar saltos de layout */
                    display: flex;
                    align-items: center;
                }
                .wave {
                    width: 100%;
                }
                .estadoCarga,
                .estadoError {
                    position: absolute;
                    left: 10px;
                    font-size: 0.75rem;
                    opacity: 0.5;
                }
            `}</style>
        </div>
    );
}
