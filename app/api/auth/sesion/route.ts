// app/api/auth/sesion/route.ts
import { NextResponse, type NextRequest } from 'next/server';

export async function GET(request: NextRequest) {
    const tokenCookie = request.cookies.get('kamples_token');

    if (!tokenCookie || !tokenCookie.value) {
        return NextResponse.json({ error: 'No autorizado, token no encontrado' }, { status: 401 });
    }

    const token = tokenCookie.value;
    const apiUrl = process.env.NEXT_PUBLIC_SWORD_API_URL;

    try {
        const respuestaApi = await fetch(`${apiUrl}/users/me`, {
            method: 'GET',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            signal: AbortSignal.timeout(5000)
        });

        if (!respuestaApi.ok) {
            const errorData = await respuestaApi.json().catch(() => ({}));
            const message = errorData?.error?.message || 'Token inválido o sesión expirada en el backend';
            const respuestaError = NextResponse.json({ error: message }, { status: respuestaApi.status });
            respuestaError.cookies.set('kamples_token', '', { expires: new Date(0), path: '/' });
            return respuestaError;
        }

        const datosUsuario = await respuestaApi.json();
        const usuario = datosUsuario.data || datosUsuario;

        return NextResponse.json({ usuario });

    } catch (error) {
        console.error("Error al verificar la sesión con el backend:", error);
        return NextResponse.json({ error: 'Error de comunicación con el servidor de autenticación' }, { status: 500 });
    }
}