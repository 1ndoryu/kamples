// app/api/samples/upload/route.ts

import {NextRequest, NextResponse} from 'next/server';
// No necesitamos la importación de 'next/headers'

export async function POST(request: NextRequest) {
    // 1. Verificar la sesión del usuario obteniendo el token directamente desde el objeto request
    const tokenCookie = request.cookies.get('kamples_token');

    if (!tokenCookie) {
        return NextResponse.json({error: {message: 'No autorizado. Debes iniciar sesión para subir un sample.'}}, {status: 401});
    }
    const token = tokenCookie.value;

    try {
        // 2. Obtener el FormData de la petición entrante (esto se mantiene igual)
        const formData = await request.formData();
        const apiUrl = process.env.NEXT_PUBLIC_SWORD_API_URL;

        // 3. Reenviar el FormData al endpoint de SwordPHP (esto se mantiene igual)
        const respuestaApi = await fetch(`${apiUrl}/samples/upload`, {
            method: 'POST',
            headers: {
                Authorization: `Bearer ${token}`
            },
            body: formData,
            signal: AbortSignal.timeout(30000) // Timeout de 30s para subidas de archivos
        });

        // 4. Analizar la respuesta del backend (esto se mantiene igual)
        const datosRespuesta = await respuestaApi.json();

        if (!respuestaApi.ok) {
            return NextResponse.json({error: datosRespuesta.error || {message: 'Error en el servidor de Kamples'}}, {status: respuestaApi.status});
        }

        return NextResponse.json(datosRespuesta, {status: 201});
    } catch (error: any) {
        console.error('Error en /api/samples/upload:', error);
        if (error.name === 'AbortError') {
            return NextResponse.json({error: {message: 'La subida del archivo ha tardado demasiado y fue cancelada.'}}, {status: 408});
        }
        return NextResponse.json({error: {message: 'Error interno del servidor al procesar la subida.'}}, {status: 500});
    }
}