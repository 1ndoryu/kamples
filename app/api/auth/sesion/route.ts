// app/api/auth/sesion/route.ts
import { NextResponse } from 'next/server';
import { cookies } from 'next/headers';
import { jwtDecode } from 'jwt-decode';

// Interface para el payload esperado en el JWT que genera tu API
interface JwtPayload {
    id: number;
    nombreusuario: string;
    nombremostrado: string;
    correoelectronico: string;
    rol: string;
    // iat, exp, etc., que son campos estándar de JWT
}

export async function GET() {
    const cookieStore = cookies();
    const tokenCookie = cookieStore.get('kamples_token');

    if (!tokenCookie) {
        return NextResponse.json({ error: 'No autorizado, token no encontrado' }, { status: 401 });
    }

    const token = tokenCookie.value;

    try {
        /*
         * ADVERTENCIA DE SEGURIDAD IMPORTANTE:
         * Esto es una simulación. jwt-decode solo decodifica el token, NO verifica su firma.
         * Un token modificado o falso sería aceptado aquí.
         *
         * SOLUCIÓN RECOMENDADA:
         * 1. Crea un endpoint en tu API de SwordPHP (ej: GET /auth/me o /auth/verify).
         * 2. Desde esta función, envía el token a tu API.
         * 3. Tu API debe verificar la firma, la expiración y devolver los datos del usuario.
         * 4. Si tu API dice que el token es válido, devuelves los datos. Si no, devuelves un error 401.
        */
        const decodificado = jwtDecode<JwtPayload>(token);

        // Creamos el objeto de usuario a partir de la información del token
        const usuario = {
            id: decodificado.id,
            nombreusuario: decodificado.nombreusuario,
            nombremostrado: decodificado.nombremostrado,
            correoelectronico: decodificado.correoelectronico,
            rol: decodificado.rol,
        };

        return NextResponse.json({ usuario });

    } catch (error) {
        console.error("Error al procesar el token de sesión:", error);
        // Si jwt-decode falla (token malformado o expirado), borramos la cookie inválida.
        const respuesta = NextResponse.json({ error: 'Token inválido o expirado' }, { status: 401 });
        respuesta.cookies.set('kamples_token', '', { expires: new Date(0), path: '/' });
        return respuesta;
    }
}