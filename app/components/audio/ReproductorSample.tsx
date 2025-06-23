// app/components/audio/ReproductorSample.tsx
'use client';

import {useState} from 'react';
import type {Sample} from '@/types/sample';
import Waveform from './Waveform';
import {useAudioPlayerStore} from '@/hooks/useAudioPlayer';

interface Props {
    sample: Sample;
}

export default function ReproductorSample({sample}: Props) {
    const [estaSonando, setEstaSonando] = useState(false);
    const {idSampleActual} = useAudioPlayerStore();
    const urlAudio = sample.metadata.url_archivo;

    if (!urlAudio) {
        return <div className="reproductorPlaceholder">Audio no disponible</div>;
    }

    const esEsteSampleElActual = idSampleActual === sample.id && estaSonando;

    return (
        <>
            <button className="botonPlayPause" aria-label={`Reproducir ${sample.titulo}`}>
                {esEsteSampleElActual ? '❚❚' : '▶'}
            </button>
            <Waveform idSample={sample.id} urlAudio={urlAudio} onPlay={() => setEstaSonando(true)} onPause={() => setEstaSonando(false)} />
            <style jsx>{`
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
                    opacity: 0.8;
                    transition: opacity 0.2s;
                }
                .botonPlayPause:hover {
                    opacity: 1;
                }
                .reproductorPlaceholder {
                    font-size: 0.75rem;
                    opacity: 0.5;
                    flex-grow: 1;
                    padding-left: 1rem;
                }
            `}</style>
        </>
    );
}
