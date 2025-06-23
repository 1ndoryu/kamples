// app/components/ListaSamples.tsx

'use client';

import {useState, useEffect} from 'react';
import type {Sample} from '@/types/sample';
import TarjetaSample from '@/components/TarjetaSample';

interface Props {
	samples: Sample[];
}

export default function ListaSamples({samples: samplesIniciales}: Props) {
	// El estado se inicializa directamente con los samples recibidos.
	// El useEffect anterior era redundante y causaba un re-render innecesario.
	const [samples, setSamples] = useState<Sample[]>(samplesIniciales);

	// Este efecto ahora solo sirve para actualizar la lista si los props cambian
	// (por ejemplo, al aplicar filtros en el futuro), no para la carga inicial.
	useEffect(() => {
		setSamples(samplesIniciales);
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
					grid-template-columns: 1fr;
					gap: 0.6rem;
				}
			`}</style>
		</>
	);
}