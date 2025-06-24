// app/components/ListaSamples.tsx

'use client';

import { useEffect } from 'react';
// import type {Sample} from '@/types/sample'; // Sample se usa en el store, aquí solo consumimos
import TarjetaSample from '@/components/TarjetaSample';
import { useAppStore } from '@/store/useAppStore';

// La prop samplesIniciales ya no es necesaria, el componente obtendrá los datos del store.
// interface Props {
//	samples: Sample[];
// }

export default function ListaSamples(/*{samples: samplesIniciales}: Props*/) {
	const samples = useAppStore((state) => state.samples);
	const isLoadingSamples = useAppStore((state) => state.isLoadingSamples);
	const errorSamples = useAppStore((state) => state.errorSamples);
	const fetchSamplesStore = useAppStore((state) => state.fetchSamplesStore);

	useEffect(() => {
		// Cargar los samples si no están ya cargados (o si se quiere recargar siempre)
		// Podríamos añadir una lógica para no recargar si ya hay samples,
		// pero fetchSamplesStore ya tiene una guarda interna para isLoadingSamples.
		if (samples.length === 0) { // Solo cargar si no hay samples en el store
			fetchSamplesStore();
		}
	}, [fetchSamplesStore, samples.length]);

	if (isLoadingSamples && samples.length === 0) { // Mostrar carga solo si no hay datos previos
		return <p>Cargando samples...</p>;
	}

	if (errorSamples) {
		return <p>Error al cargar samples: {errorSamples}</p>;
	}

	return (
		<>
			{samples.length > 0 ? (
				<div className="rejillaSamples">
					{samples.map((sample) => ( // Usar sample.id que debería ser único
						<TarjetaSample key={sample.id} sample={sample} />
					))}
				</div>
			) : (
				// Si no hay loading y no hay error, pero no hay samples, mostrar mensaje.
				!isLoadingSamples && <p>No se encontraron samples.</p>
			)}

			<style jsx>{`
				.rejillaSamples {
					display: grid;
					grid-template-columns: 1fr;
					gap: 0.6rem;
				}
			`}</style>
		</>
	);
}