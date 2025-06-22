// app/samples/[slug]/page.tsx (Versión de Depuración)
import type { Metadata } from 'next';

// Comentario de Depuración:
// Se usa la definición de props estándar recomendada por Next.js.
// Incluye 'params' y 'searchParams' para cumplir con la restricción 'PageProps'.
type Props = {
    params: { slug: string };
    searchParams: { [key: string]: string | string[] | undefined };
}

// Comentario de Depuración:
// Se simplifica generateMetadata para no depender de llamadas a la API.
// Esto asegura que un error en `obtenerSamplePorSlug` no afecte la inferencia de tipos.
export async function generateMetadata({ params }: Props): Promise<Metadata> {
    return {
        title: `Depurando Sample: ${params.slug}`
    };
}

// Comentario de Depuración:
// El componente se ha reducido a su mínima expresión. No usa Suspense,
// no llama a otros componentes, no hace fetching de datos. Su único
// propósito es renderizar el 'slug' para verificar que los props se reciben
// y tipan correctamente durante el build.
export default function PaginaDeSample({ params }: Props) {
    return (
        <main style={{ padding: '2rem' }}>
            <h1>Modo de Depuración</h1>
            <p>Si ves esta página, el tipado básico funciona.</p>
            <p>Slug del sample: <strong>{params.slug}</strong></p>
        </main>
    );
}