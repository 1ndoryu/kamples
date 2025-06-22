// app/api/auth/login/route.ts
import {NextResponse} from 'next/server';

export async function POST(request: Request) {
    try {
        const {nombreUsuario, clave} = await request.json();
        const baseUrl = process.env.NEXT_PUBLIC_SWORD_BASE_URL;

        // 1. Llamamos a tu API de SwordPHP desde nuestro servidor
        const respuestaApi = await fetch(`${baseUrl}/auth/token`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({nombreusuario: nombreUsuario, clave: clave})
        });

        const datos = await respuestaApi.json();

        // 2. Si las credenciales son incorrectas, tu API devuelve un error
        if (!respuestaApi.ok) {
            return NextResponse.json({error: datos.error.message || 'Error de autenticación'}, {status: respuestaApi.status});
        }

        // 3. Si todo va bien, extraemos el token y los datos del usuario
        const {token, usuario} = datos.data;

        // 4. Creamos una respuesta y guardamos el token en una cookie segura
        const respuesta = NextResponse.json({usuario});

        respuesta.cookies.set('kamples_token', token, {
            httpOnly: true, // El cliente JS no puede leerla
            secure: process.env.NODE_ENV === 'production', // Solo por HTTPS en producción
            maxAge: 60 * 60 * 24 * 7, // 1 semana
            path: '/' // Disponible en toda la web
        });

        return respuesta;
    } catch (error) {
        console.error('Error en /api/auth/login:', error);
        return NextResponse.json({error: 'Error interno del servidor'}, {status: 500});
    }
}
