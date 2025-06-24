// app/page.tsx

// import { obtenerSamples } from '@/services/swordApi'; // Ya no es necesario aquí
// import type { Sample } from '@/types/sample'; // Ya no es necesario aquí
import ListaSamples from '@/components/ListaSamples';

// Esta página puede seguir siendo un Server Component, aunque no cargue datos directamente.
// O podría convertirse en un Client Component si fuera necesario por otras razones.
// Por ahora, la mantenemos como Server Component que simplemente renderiza ListaSamples.
export default function PaginaPrincipal() {
    // La carga de datos ahora es manejada por ListaSamples y el store de Zustand en el cliente.
    // const samples: Sample[] = await obtenerSamples(); // Eliminado

    return (
        <section className="contenedorPrincipal">
            {/* ListaSamples ahora carga sus propios datos desde el store */}
            <ListaSamples />
            
            {/* El bloque <style jsx> ha sido eliminado de aquí */}
        </section>
    );
}