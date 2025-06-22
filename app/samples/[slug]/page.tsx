// app/samples/[slug]/page.tsx
import {Suspense} from 'react';
import {notFound} from 'next/navigation';
import {obtenerSamplePorSlug} from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type {Metadata} from 'next';
import type {Sample} from '@/types/sample';

// 1. (CORRECCIÓN) Creamos una interfaz para los parámetros
interface PaginaSampleProps {
	params: {
		slug: string;
	};
}

// 2. (CORRECCIÓN) Usamos la interfaz para tipar los 'params'
export async function generateMetadata({params}: PaginaSampleProps): Promise<Metadata> {
	const sample: Sample | null = await obtenerSamplePorSlug(params.slug);

	if (!sample) {
		return {title: 'Sample no encontrado'};
	}
	return {
		title: `${sample.titulo} - Kamples`,
		description: sample.subtitulo || sample.contenido?.substring(0, 150) || ''
	};
}

// Componente asíncrono para la carga de datos.
async function SampleLoader({slug}: {slug: string}) {
	const sample = await obtenerSamplePorSlug(slug);
	if (!sample) {
		notFound();
	}
	return <DetalleSample sample={sample} />;
}

// 3. (CORRECCIÓN) Tipamos los 'params' también en el componente de la página
export default function PaginaDeSample({params}: PaginaSampleProps) {
	return (
		<div>
			<Suspense fallback={<div className="cargandoContenido">Cargando sample...</div>}>
				<SampleLoader slug={params.slug} />
			</Suspense>
		</div>
	);
}