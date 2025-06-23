// app/hooks/useAudioPlayer.ts
import {create} from 'zustand';

interface AudioPlayerState {
    instanciaActual: any | null; // La instancia de WaveSurfer que está sonando
    idSampleActual: number | null;
    reproducir: (instancia: any, idSample: number) => void;
    detener: () => void;
}

export const useAudioPlayerStore = create<AudioPlayerState>((set, get) => ({
    instanciaActual: null,
    idSampleActual: null,
    reproducir: (instancia, idSample) => {
        const {instanciaActual} = get();
        if (instanciaActual && instanciaActual !== instancia) {
            instanciaActual.pause();
        }
        set({instanciaActual: instancia, idSampleActual: idSample});
        instancia.play();
    },
    detener: () => {
        const {instanciaActual} = get();
        if (instanciaActual) {
            instanciaActual.pause();
        }
        set({instanciaActual: null, idSampleActual: null});
    }
}));
