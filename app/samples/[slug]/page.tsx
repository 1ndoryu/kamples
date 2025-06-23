// app/samples/[slug]/page.tsx
// TEST: Aplicando la solución de Stack Overflow para params como Promise.

import { Suspense } from 'react';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type { Metadata } from 'next';

// 1. Modificamos el tipado para que acepte una Promise, como sugiere el post.
type PageProps = {
    params: Promise<{ slug: string }>;
};

// 2. generateMetadata ahora debe esperar (await) a que se resuelvan los params.
export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
    const { slug } = await params; // Se resuelve la Promise
    const sample = await obtenerSamplePorSlug(slug);

    if (!sample) {
        return { title: 'Sample no encontrado' };
    }
    return {
        title: `${sample.titulo} - Kamples`,
        description: sample.subtitulo || sample.contenido?.substring(0, 150) || '',
    };
}

// El SampleLoader no necesita cambios, ya que le pasaremos el slug resuelto.
async function SampleLoader({ slug }: { slug: string }) {
    const sample = await obtenerSamplePorSlug(slug);
    if (!sample) {
        notFound();
    }
    return <DetalleSample sample={sample} />;
}

// 3. El componente principal también debe ser async y esperar (await) los params.
export default async function PaginaDeSample({ params }: PageProps) {
    const { slug } = await params; // Se resuelve la Promise

    return (
        <div>
            <Suspense fallback={<div className="cargandoContenido">Cargando sample...</div>}>
                <SampleLoader slug={slug} />
            </Suspense>
        </div>
    );
}