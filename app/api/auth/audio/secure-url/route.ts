// app/api/audio/secure-url/route.ts
import {NextRequest, NextResponse} from 'next/server';
import {generarTokenAudio} from '@/lib/authAudio';
import {cookies} from 'next/headers';

export async function GET(request: NextRequest) {
    // Verificamos si el usuario está logueado
    const tokenCookie = cookies().get('kamples_token');
    if (!tokenCookie) {
        return NextResponse.json({error: 'No autorizado'}, {status: 401});
    }

    const {searchParams} = new URL(request.url);
    const mediaUrl = searchParams.get('mediaUrl');

    if (!mediaUrl) {
        return NextResponse.json({error: 'Falta el parámetro mediaUrl'}, {status: 400});
    }

    try {
        const token = await generarTokenAudio({mediaUrl});
        // Construimos la URL que apunta a nuestro endpoint de streaming
        const secureUrl = `/api/audio/stream/${token}`;
        return NextResponse.json({secureUrl});
    } catch (error) {
        console.error('Error generando token de audio:', error);
        return NextResponse.json({error: 'Error interno del servidor'}, {status: 500});
    }
}
