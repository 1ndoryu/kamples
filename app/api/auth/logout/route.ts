// app/api/auth/logout/route.ts
import {NextResponse} from 'next/server';

export async function POST() {
    // Para cerrar sesión, simplemente eliminamos la cookie
    const respuesta = NextResponse.json({mensaje: 'Sesión cerrada'});

    respuesta.cookies.set('kamples_token', '', {
        httpOnly: true,
        secure: process.env.NODE_ENV === 'production',
        expires: new Date(0), // Fija la fecha de expiración en el pasado
        path: '/'
    });

    return respuesta;
}
