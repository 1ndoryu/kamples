// app/page.tsx

import { obtenerSamples } from '@/services/swordApi';
import type { Sample } from '@/types/sample';
import ListaSamples from '@/components/ListaSamples';

// Esta página sigue siendo un Server Component, ideal para el SEO.
export default async function PaginaPrincipal() {
    // Se mantiene la obtención de datos en el servidor
    const samples: Sample[] = await obtenerSamples();

    return (
        <section className="contenedorPrincipal">
            <h1>Biblioteca de Samples</h1>
            <p>Estos son los últimos samples subidos.</p>

            {/* Usamos el componente cliente para mostrar los datos */}
            <ListaSamples samples={samples} />
            
            {/* El bloque <style jsx> ha sido eliminado de aquí */}
        </section>
    );
}