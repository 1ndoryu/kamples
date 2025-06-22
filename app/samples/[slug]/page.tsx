// app/samples/[slug]/page.tsx
import { Suspense } from 'react';
import { notFound } from 'next/navigation';
import { obtenerSamplePorSlug } from '@/services/swordApi';
import DetalleSample from '@/components/DetalleSample';
import type { Sample } from '@/types/sample';

// PASO 1: Se crea un nuevo componente ASÍNCRONO para cargar los datos.
// Al no ser el export por defecto de la página, sus props no entran
// en conflicto con los tipos autogenerados por Next.js.
async function SampleLoader({ slug }: { slug: string }) {
    const sample: Sample | null = await obtenerSamplePorSlug(slug);

    if (!sample) {
        notFound();
    }

    return <DetalleSample sample={sample} />;
}

// PASO 2: El componente principal de la página ahora es SÍNCRONO.
// Esto evita el bug del compilador que afecta a las props de las páginas asíncronas.
// Su única 'prop' es la que le pasa el sistema de enrutamiento.
export default function PaginaDeSample({ params }: { params: { slug: string } }) {
    return (
        <div>
            {/* Suspense se encarga de mostrar un fallback mientras el componente 
              asíncrono SampleLoader espera la respuesta de la API.
            */}
            <Suspense fallback={<div>Cargando sample...</div>}>
                <SampleLoader slug={params.slug} />
            </Suspense>
        </div>
    );
}

// NOTA: Se ha eliminado la función `generateMetadata` para aislar el problema,
// ya que probablemente sufría del mismo bug de tipado. Se puede reintroducir
// más adelante una vez que la compilación principal funcione.