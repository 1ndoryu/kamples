// app/api/audio/stream/[token]/route.ts
import {NextRequest, NextResponse} from 'next/server';
import {verificarTokenAudio} from '@/lib/authAudio';

export async function GET(request: NextRequest, {params}: {params: {token: string}}) {
    const {token} = params;

    if (!token) {
        return new Response('Token no proporcionado', {status: 400});
    }

    try {
        const payload = await verificarTokenAudio(token);
        const baseUrl = process.env.NEXT_PUBLIC_SWORD_BASE_URL;
        const audioUrl = `${baseUrl}${payload.mediaUrl}`;

        // Hacemos fetch al audio original desde nuestro servidor para no exponer la URL directa
        const audioResponse = await fetch(audioUrl, {
            headers: {
                // Si tu CMS requiere alguna auth para acceder a los archivos, añádela aquí
                Referer: baseUrl
            }
        });

        if (!audioResponse.ok || !audioResponse.body) {
            throw new Error(`No se pudo obtener el archivo de audio desde ${audioUrl}`);
        }

        // Obtenemos los headers originales para pasarlos al cliente (importante para el streaming)
        const headers = new Headers();
        headers.set('Content-Type', audioResponse.headers.get('Content-Type') || 'audio/mpeg');
        headers.set('Content-Length', audioResponse.headers.get('Content-Length') || '0');
        headers.set('Accept-Ranges', 'bytes');
        headers.set('Cache-Control', 'public, max-age=604800, immutable'); // Cachear en el navegador

        // Devolvemos el stream del audio
        return new Response(audioResponse.body, {
            status: 200,
            headers: headers
        });
    } catch (error) {
        console.error('Error en el stream de audio:', error);
        return new Response('Token inválido o expirado.', {status: 403});
    }
}
