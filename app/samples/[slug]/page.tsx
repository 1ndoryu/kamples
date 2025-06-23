import {Suspense} from 'react';
import {notFound} from 'next/navigation';
import {obtenerSamplePorSlug} from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type {Metadata} from 'next';

interface PaginaSampleProps {
    params: {slug: string};
    searchParams: {[key: string]: string | string[] | undefined};
}

export async function generateMetadata({params}: PaginaSampleProps): Promise<Metadata> {
    const sample = await obtenerSamplePorSlug(params.slug);
    if (!sample) {
        return {title: 'Sample no encontrado'};
    }
    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido?.substring(0, 150) || ''
    };
}

async function SampleLoader({slug}: {slug: string}) {
    const sample = await obtenerSamplePorSlug(slug);
    if (!sample) notFound();
    return <DetalleSample sample={sample} />;
}

export default function PaginaDeSample({params, searchParams}: PaginaSampleProps) {
    return (
        <div>
            <Suspense fallback={<div className="cargandoContenido">Cargando sample...</div>}>
                <SampleLoader slug={params.slug} />
            </Suspense>
        </div>
    );
}
