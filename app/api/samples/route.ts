import { NextResponse } from 'next/server';
// Asumiremos que swordApi.ts tiene o podemos añadir una función para crear samples,
// o usaremos una versión adaptada de fetchApi directamente aquí.
// Por ahora, vamos a simular la creación y la interacción con una hipotética función de swordApi.

// import { crearSampleEnSword } from '@/services/swordApi'; // Hipotética función
import type { Sample, MetadataSample } from '@/types/sample'; // Asegúrate de que los tipos son correctos

// Simulación de la función que estaría en swordApi.ts o se construiría con fetchApi
async function crearSampleEnSwordAPI(datosSample: { titulo: string; subtitulo?: string; contenido?: string; metadata: MetadataSample, tipocontenido: string, estado?: string }): Promise<Sample> {
    // Aquí iría la lógica real para llamar al backend de Sword usando fetchApi
    // const endpoint = '/content'; // O el endpoint correcto para crear
    // const options = {
    //     method: 'POST',
    //     body: JSON.stringify(datosSample),
    // };
    // return fetchApi<Sample>(endpoint, options);

    console.log("API Route: Datos recibidos para crear sample:", datosSample);
    // Simulación de respuesta exitosa de la API externa
    const nuevoSampleSimulado: Sample = {
        id: Math.floor(Math.random() * 10000),
        titulo: datosSample.titulo,
        subtitulo: datosSample.subtitulo || null,
        contenido: datosSample.contenido || '',
        slug: datosSample.titulo.toLowerCase().replace(/\s+/g, '-'), // Slug simple simulado
        idautor: 1, // Simulado
        estado: 'publicado', // Simulado
        tipocontenido: datosSample.tipocontenido || 'sample',
        metadata: datosSample.metadata,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
    };
    console.log("API Route: Nuevo sample simulado:", nuevoSampleSimulado);
    return Promise.resolve(nuevoSampleSimulado);
}


export async function POST(request: Request) {
    try {
        const formData = await request.formData();
        console.log("API Route (POST /api/samples): FormData recibida.");

        const titulo = formData.get('titulo') as string;
        const tipocontenido = formData.get('tipocontenido') as string || 'sample';
        const estado = formData.get('estado') as string || 'publicado';
        const postTagsRaw = formData.get('post_tags') as string; // JSON string de tags
        // const contenidoRaw = formData.get('contenido') as string; // 'contenido' de tu textarea

        const archivoSample = formData.get('archivoSample') as File | null;
        const archivoImagen = formData.get('archivoImagen') as File | null;

        // Aquí necesitarías procesar 'postTagsRaw' (parsear JSON) y construir el objeto 'metadata'
        // que espera 'crearSampleEnSwordAPI'.
        // Por ahora, lo simularé.
        let tags = [];
        if (postTagsRaw) {
            try {
                tags = JSON.parse(postTagsRaw);
            } catch (e) { console.error("Error parseando post_tags", e); }
        }

        const metadata: MetadataSample = {
             // Ejemplo: extrae otros campos de metadata si vienen en formData
            tags: tags,
            // bpm: formData.get('bpm') as string || undefined,
            // ... otros campos de metadata
        };

        // Validación básica
        if (!titulo || !archivoSample) {
            return NextResponse.json({ error: 'Título y archivo de audio son requeridos' }, { status: 400 });
        }

        console.log("API Route: Titulo:", titulo);
        console.log("API Route: Archivo Sample:", archivoSample?.name, archivoSample?.size);
        if (archivoImagen) {
            console.log("API Route: Archivo Imagen:", archivoImagen.name, archivoImagen.size);
        }
        console.log("API Route: Metadata a enviar (simulada/parcial):", metadata);


        // Simulación de pasar datos a la función de API externa
        // En una implementación real, pasarías los archivos (o sus buffers/streams) y los metadatos
        const datosParaSword = {
            titulo,
            tipocontenido,
            estado,
            metadata, // Metadata construida
            // contenido: contenidoRaw, // Si lo usas
            // Aquí también necesitarías manejar los archivos `archivoSample` y `archivoImagen`
            // y pasarlos a `crearSampleEnSwordAPI` de la forma que espere (ej. como buffers, o subirlos a S3 y pasar URLs)
        };

        // Llamada a la función simulada (que espera JSON, no archivos directamente)
        // En la realidad, crearSampleEnSwordAPI necesitaría manejar FormData o los archivos procesados.
        const nuevoSample = await crearSampleEnSwordAPI(datosParaSword);
        return NextResponse.json(nuevoSample, { status: 201 });

    } catch (error: any) {
        console.error('Error en POST /api/samples:', error);
        const message = error.message || 'Error al crear el sample';
        return NextResponse.json({ error: message }, { status: 500 });
    }
}

// También podríamos añadir una función GET aquí si quisiéramos que /api/samples
// sirva la lista de samples, en lugar de que el store llame directamente a swordApi.
// Por consistencia, podría ser una buena idea.
// import { obtenerSamples } from '@/services/swordApi';
// export async function GET() {
//     try {
//         const samples = await obtenerSamples();
//         return NextResponse.json(samples);
//     } catch (error: any) {
//         console.error('Error en GET /api/samples:', error);
//         return NextResponse.json({ error: error.message || 'Error al obtener samples' }, { status: 500 });
//     }
// }
