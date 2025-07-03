'use client';

import {create} from 'zustand';
import WaveSurfer from 'wavesurfer.js';

type WaveInstance = WaveSurfer;

interface AudioPlayerState {
  idSampleActual: number | null;
  instanciaActual: WaveInstance | null;
  transicionando: boolean;
  /** Instancias registradas para cada sample */
  instanciasPorSample: Record<number, WaveInstance[]>;
  registrarInstancia: (idSample: number, instancia: WaveInstance) => void;
  eliminarInstancia: (idSample: number, instancia: WaveInstance) => void;
  /** Sincroniza la posición de reproducción de la instancia principal con el resto */
  _sincronizarInstancias: (idSample: number, instanciaPrincipal: WaveInstance) => void;
  alternarReproduccion: (instancia: WaveInstance, idSample: number) => void;
  detenerPorCompleto: () => void;
}

export const useAudioPlayerStore = create<AudioPlayerState>((set, get) => ({
  idSampleActual: null,
  instanciaActual: null,
  transicionando: false,
  /** Instancias registradas para cada sample */
  instanciasPorSample: {},

  registrarInstancia(idSample, instancia) {
    set(state => {
      const arr = state.instanciasPorSample[idSample] ?? [];
      if (!arr.includes(instancia)) arr.push(instancia);
      return {instanciasPorSample: {...state.instanciasPorSample, [idSample]: arr}};
    });
  },

  eliminarInstancia(idSample, instancia) {
    set(state => {
      const arr = (state.instanciasPorSample[idSample] ?? []).filter(i => i !== instancia);
      const nuevoMapa = {...state.instanciasPorSample};
      if (arr.length > 0) {
        nuevoMapa[idSample] = arr;
      } else {
        delete nuevoMapa[idSample];
      }
      return {instanciasPorSample: nuevoMapa};
    });
  },

  _sincronizarInstancias(idSample, instanciaPrincipal) {
    const {instanciasPorSample} = get();

    const otrasInstancias = (instanciasPorSample[idSample] ?? []).filter(i => i !== instanciaPrincipal);

    // Al recibir audioprocess del principal, actualizar las otras instancias
    const sincronizar = () => {
      const dur = instanciaPrincipal.getDuration();
      if (!dur) return;
      const pos = instanciaPrincipal.getCurrentTime() / dur;
      otrasInstancias.forEach(inst => {
        inst.seekTo(pos);
      });
    };

    // Limpiar posibles escuchas previas
    // Nota: Desvinculamos la función si estaba previamente registrada
    // @ts-ignore -- off puede no estar tipado en algunas versiones
    (instanciaPrincipal as any).off?.('audioprocess', sincronizar);
    instanciaPrincipal.on('audioprocess', sincronizar);
  },

  alternarReproduccion(instancia, idSample) {
    const {idSampleActual, instanciaActual} = get();

    // Si es el mismo sample actualmente activo, alternar play/pause
    if (idSampleActual === idSample && instanciaActual) {
      if (instanciaActual.isPlaying()) {
        instanciaActual.pause();
      } else {
        instanciaActual.play();
      }
      // Forzar re-render para que los componentes actualicen su estado visual
      set(state => ({...state}));
      return;
    }

    // Vamos a cambiar de sample
    set({transicionando: true});

    // Detener la instancia previa completamente
    if (instanciaActual) {
      instanciaActual.pause();
      instanciaActual.seekTo(0);
    }

    // Pausar posibles otras instancias del mismo sample para evitar duplicar audio
    const {instanciasPorSample} = get();
    const otrasDelMismoSample = (instanciasPorSample[idSample] ?? []).filter(i => i !== instancia);
    otrasDelMismoSample.forEach(i => i.pause());

    // Reproducir el nuevo sample
    instancia.play();

    set(state => {
      // Activar sincronización entre instancias de este sample
      state._sincronizarInstancias(idSample, instancia);

      return {
        idSampleActual: idSample,
        instanciaActual: instancia,
        transicionando: false
      } as any;
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