// app/api/auth/audio/secure-url/route.ts
import {NextRequest, NextResponse} from 'next/server';
import {generarTokenAudio} from '@/lib/authAudio';

export async function GET(request: NextRequest) {
	console.log('API /api/audio/secure-url RECIBIÓ UNA PETICIÓN');

	const tokenCookie = request.cookies.get('kamples_token');
	if (!tokenCookie) {
		console.log('Rechazado: No hay cookie de sesión (kamples_token)');
		return NextResponse.json({error: 'No autorizado'}, {status: 401});
	}

	const {searchParams} = new URL(request.url);
	const mediaUrl = searchParams.get('mediaUrl');

	if (!mediaUrl) {
		console.log('Rechazado: Falta el parámetro mediaUrl');
		return NextResponse.json({error: 'Falta el parámetro mediaUrl'}, {status: 400});
	}

	console.log(`Petición para generar token para: ${mediaUrl}`);

	try {
		const token = await generarTokenAudio({mediaUrl});

		// --- REFACTORIZACIÓN ---
		// Ahora construimos la URL con el token como un parámetro de consulta.
		const secureUrl = `/api/auth/audio/stream?token=${token}`;

		console.log('Token generado. URL segura creada:', secureUrl);
		return NextResponse.json({secureUrl});
	} catch (error) {
		console.error('Error generando token de audio:', error);
		return NextResponse.json({error: 'Error interno del servidor al generar token'}, {status: 500});
	}
}