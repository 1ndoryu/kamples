// app/api/auth/audio/stream/route.ts
import {NextRequest} from 'next/server';
import {verificarTokenAudio} from '@/lib/authAudio';

export async function GET(request: NextRequest) {
	const {searchParams} = new URL(request.url);
	const token = searchParams.get('token');

	if (!token) {
		return new Response('Token no proporcionado', {status: 400});
	}

	try {
		const payload = await verificarTokenAudio(token);
		const baseUrl = process.env.NEXT_PUBLIC_SWORD_BASE_URL;

		if (!baseUrl) {
			throw new Error('La variable de entorno NEXT_PUBLIC_SWORD_BASE_URL no está configurada.');
		}

		const audioUrl = `${baseUrl}${payload.mediaUrl}`;

		const headersBackend = new Headers();

		// --- AÑADIDO CLAVE ---
		// Añadimos la cabecera Referer. Tu backend probablemente la necesita
		// para verificar que la petición viene de un origen permitido.
		headersBackend.set('Referer', baseUrl);
		
		const rangeHeader = request.headers.get('range');
		if (rangeHeader) {
			headersBackend.set('range', rangeHeader);
		}

		const audioResponse = await fetch(audioUrl, {
			headers: headersBackend,
			cache: 'no-store'
		});

		if (!audioResponse.ok) {
			return new Response(audioResponse.body, {
				status: audioResponse.status,
				statusText: audioResponse.statusText,
				headers: audioResponse.headers
			});
		}

		const responseHeaders = new Headers();
		responseHeaders.set('Content-Type', audioResponse.headers.get('Content-Type') || 'audio/mpeg');
		responseHeaders.set('Accept-Ranges', audioResponse.headers.get('Accept-Ranges') || 'bytes');

		const contentLength = audioResponse.headers.get('Content-Length');
		if (contentLength) {
			responseHeaders.set('Content-Length', contentLength);
		}

		const contentRange = audioResponse.headers.get('Content-Range');
		if (rangeHeader && contentRange) {
			responseHeaders.set('Content-Range', contentRange);
		}

		return new Response(audioResponse.body, {
			status: audioResponse.status,
			statusText: audioResponse.statusText,
			headers: responseHeaders
		});
		
	} catch (error) {
		const message = error instanceof Error ? error.message : 'Error desconocido en stream.';
		console.error('Error en el stream de audio:', message);
		return new Response(message, {status: 500});
	}
}