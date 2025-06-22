// app/api/auth/samples/upload/route.ts
import { NextRequest, NextResponse } from 'next/server';

export async function POST(request: NextRequest) {
    // 1. Verificar la sesión del usuario obteniendo el token.
    const tokenCookie = request.cookies.get('kamples_token');
    if (!tokenCookie) {
        return NextResponse.json({ error: { message: 'No autorizado. Debes iniciar sesión para subir un sample.' } }, { status: 401 });
    }
    const token = tokenCookie.value;

    try {
        const formData = await request.formData();
        const apiUrl = process.env.NEXT_PUBLIC_SWORD_API_URL;

        // 2. Reenviar el FormData al endpoint de SwordPHP.
        const respuestaApi = await fetch(`${apiUrl}/samples/upload`, {
            method: 'POST',
            headers: {
                Authorization: `Bearer ${token}`,
            },
            body: formData,
            signal: AbortSignal.timeout(30000), 
        });


        // --- AQUÍ ESTÁ LA LÓGICA QUE CONTIENE EL CONSOLE.ERROR ---
        // 3. Verificamos si la respuesta del backend es JSON antes de procesarla.
        const contentType = respuestaApi.headers.get('content-type');

        if (!contentType || !contentType.includes('application/json')) {
            // Si NO es JSON, es porque PHP falló y devolvió un error HTML.
            const respuestaTexto = await respuestaApi.text(); // Leemos la respuesta como texto.
            
            // ¡ESTA ES LA LÍNEA CLAVE QUE BUSCAS!
            // Imprime el error de PHP en la consola de tu servidor Next.js (la terminal).
            console.error('La API de SwordPHP no devolvió JSON. Respuesta recibida:', respuestaTexto);
            
            // Y luego enviamos un error controlado (502) al navegador.
            return NextResponse.json(
                {
                    error: {
                        message: 'Error inesperado del servidor backend. No se recibió una respuesta JSON válida.',
                        backendResponse: respuestaTexto.substring(0, 500),
                    },
                },
                { status: 502 }
            );
        }
        // --- FIN DE LA LÓGICA DE VERIFICACIÓN ---
        
        // Si el código llega aquí, es porque la respuesta SÍ fue JSON.
        const datosRespuesta = await respuestaApi.json();

        if (!respuestaApi.ok) {
            return NextResponse.json({ error: datosRespuesta.error || { message: 'Error en el servidor de Kamples' } }, { status: respuestaApi.status });
        }

        return NextResponse.json(datosRespuesta, { status: 201 });

    } catch (error: any) {
        console.error('Error en /api/samples/upload:', error);
        if (error.name === 'AbortError') {
            return NextResponse.json({ error: { message: 'La subida del archivo ha tardado demasiado y fue cancelada.' } }, { status: 408 });
        }
        return NextResponse.json({ error: { message: 'Error interno del servidor al procesar la subida.' } }, { status: 500 });
    }
}