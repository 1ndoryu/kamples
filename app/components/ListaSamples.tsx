// app/components/ListaSamples.tsx
'use client';

import {useState, useEffect} from 'react';
import type {Sample} from '@/types/sample';
import TarjetaSample from '@/components/TarjetaSample';

interface Props {
	samples: Sample[];
}

export default function ListaSamples({samples: samplesIniciales}: Props) {
	const [samples, setSamples] = useState<Sample[]>(samplesIniciales);

	useEffect(() => {
		// Como solicitaste, si la API devuelve un solo sample, lo duplicamos 10 veces para el desarrollo.
		if (samplesIniciales && samplesIniciales.length === 1) {
			const sampleBase = samplesIniciales[0];
			const samplesDuplicados = Array.from({length: 10}, () => sampleBase);
			setSamples(samplesDuplicados);
		} else {
			setSamples(samplesIniciales);
		}
	}, [samplesIniciales]);

	return (
		<>
			{samples.length > 0 ? (
				<div className="rejillaSamples">
					{/* Usamos el índice en la key para asegurar unicidad durante la duplicación temporal */}
					{samples.map((sample, index) => (
						<TarjetaSample key={`${sample.id}-${index}`} sample={sample} />
					))}
				</div>
			) : (
				<p>No se encontraron samples en este momento. Intenta de nuevo más tarde.</p>
			)}

			<style jsx>{`
				.rejillaSamples {
					display: grid;
					grid-template-columns: 1fr; /* Una sola columna por defecto */
					gap: 1rem;
					margin-top: 2rem;
				}
			`}</style>
		</>
	);
}