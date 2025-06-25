import { create } from 'zustand';
import type WaveSurfer from 'wavesurfer.js';

interface AudioPlayerState {
	instanciaActual: WaveSurfer | null;
	idSampleActual: number | null;
	transicionando: boolean;
	alternarReproduccion: (nuevaInstancia: WaveSurfer, nuevoIdSample: number) => Promise<void>;
	detenerPorCompleto: () => void;
}

export const useAudioPlayerStore = create<AudioPlayerState>((set, get) => ({
	instanciaActual: null,
	idSampleActual: null,
	transicionando: false,

	alternarReproduccion: async (nuevaInstancia, nuevoIdSample) => {
		const {instanciaActual, idSampleActual, transicionando} = get();

		if (transicionando) return;

		set({transicionando: true});

		try {
			if (instanciaActual && idSampleActual === nuevoIdSample) {
				await instanciaActual.playPause();
			} else {
				if (instanciaActual) {
					instanciaActual.pause();
				}
				
				set({
					instanciaActual: nuevaInstancia, 
					idSampleActual: nuevoIdSample
				});
				await nuevaInstancia.play();
			}
		} catch (error) {
			console.error('Error al alternar reproducción:', error);
			set({ instanciaActual: null, idSampleActual: null });
		} finally {
			set({transicionando: false});
		}
	},

	detenerPorCompleto: () => {
		get().instanciaActual?.seekTo(0);
		set({instanciaActual: null, idSampleActual: null, transicionando: false});
	},
}));