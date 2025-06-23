// app/hooks/useAudioPlayer.ts
import {create} from 'zustand';
import type WaveSurfer from 'wavesurfer.js';

interface AudioPlayerState {
	instanciaActual: WaveSurfer | null;
	idSampleActual: number | null;
	// NUEVO: Estado para saber si un audio está en proceso de play/pause.
	// Esto previene el error de interrupción por clics rápidos.
	transicionando: boolean;
	// ACCIÓN REFACTORIZADA: Es 'async' para manejar la promesa de play().
	alternarReproduccion: (nuevaInstancia: WaveSurfer, nuevoIdSample: number) => Promise<void>;
	detenerPorCompleto: () => void;
}

export const useAudioPlayerStore = create<AudioPlayerState>((set, get) => ({
	instanciaActual: null,
	idSampleActual: null,
	transicionando: false,

	alternarReproduccion: async (nuevaInstancia, nuevoIdSample) => {
		const {instanciaActual, idSampleActual, transicionando} = get();

		// Si ya hay una acción en curso, no hacer nada para evitar errores.
		if (transicionando) return;

		set({transicionando: true});

		try {
			// CASO 1: Clic en el mismo sample que está activo.
			if (instanciaActual && idSampleActual === nuevoIdSample) {
				await instanciaActual.playPause();
			} else {
				// CASO 2: Clic en un nuevo sample.
				if (instanciaActual) {
					instanciaActual.pause();
				}
				// Establecer el nuevo como actual y reproducirlo.
				set({instanciaActual: nuevaInstancia, idSampleActual: nuevoIdSample});
				await nuevaInstancia.play();
			}
		} catch (error) {
			// El error "interrupted" es capturado aquí si la lógica fallara.
			console.error('Error al alternar reproducción:', error);
			// Reseteamos el estado para evitar un bloqueo visual.
			set({instanciaActual: null, idSampleActual: null});
		} finally {
			// Al final, sea éxito o fallo, terminamos la transición.
			set({transicionando: false});
		}
	},

	detenerPorCompleto: () => {
		// Al terminar, nos aseguramos que el cursor del waveform vuelva al inicio.
		get().instanciaActual?.seekTo(0);
		set({instanciaActual: null, idSampleActual: null, transicionando: false});
	},
}));