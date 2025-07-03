'use client';

import {create} from 'zustand';
import WaveSurfer from 'wavesurfer.js';

type WaveInstance = WaveSurfer;

interface AudioPlayerState {
  idSampleActual: number | null;
  instanciaActual: WaveInstance | null;
  transicionando: boolean;
  alternarReproduccion: (instancia: WaveInstance, idSample: number) => void;
  detenerPorCompleto: () => void;
}

export const useAudioPlayerStore = create<AudioPlayerState>((set, get) => ({
  idSampleActual: null,
  instanciaActual: null,
  transicionando: false,

  alternarReproduccion(instancia, idSample) {
    const {idSampleActual, instanciaActual} = get();

    // Si es el mismo sample actualmente activo, alternar play/pause
    if (idSampleActual === idSample && instanciaActual) {
      if (instanciaActual.isPlaying()) {
        instanciaActual.pause();
      } else {
        instanciaActual.play();
      }
      return;
    }

    // Vamos a cambiar de sample
    set({transicionando: true});

    // Detener la instancia previa completamente
    if (instanciaActual) {
      instanciaActual.pause();
      instanciaActual.seekTo(0);
    }

    // Reproducir el nuevo sample
    instancia.play();

    set({
      idSampleActual: idSample,
      instanciaActual: instancia,
      transicionando: false
    });
  },

  detenerPorCompleto() {
    const {instanciaActual} = get();
    if (instanciaActual) {
      instanciaActual.pause();
      instanciaActual.seekTo(0);
    }
    set({idSampleActual: null, instanciaActual: null, transicionando: false});
  }
})); 