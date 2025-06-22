// app/api/auth/sesion/route.ts
import { NextResponse, type NextRequest } from 'next/server';

export async function GET(request: NextRequest) {
    // Se cambia el método de obtención de la cookie para mayor robustez,
    // usando el objeto `request` que recibe la función.
    const tokenCookie = request.cookies.get('kamples_token');

    if (!tokenCookie || !tokenCookie.value) {
        return NextResponse.json({ error: 'No autorizado, token no encontrado' }, { status: 401 });
    }

    const token = tokenCookie.value;
    const apiUrl = process.env.NEXT_PUBLIC_SWORD_API_URL;

    try {
        // --- CAMBIO FUNDAMENTAL ---
        // Ya no decodificamos el token. Lo enviamos a un nuevo endpoint en el backend
        // para que sea él quien verifique la sesión y nos devuelva los datos.
        const respuestaApi = await fetch(`${apiUrl}/users/me`, {
            method: 'GET',
            headers: {
                'Authorization': `Bearer ${token}`,
                'Content-Type': 'application/json'
            },
            // Buena práctica: añadir un timeout para evitar que se quede colgado.
            signal: AbortSignal.timeout(5000)
        });

        // Si el backend dice que el token no es válido (ej. devuelve 401),
        // replicamos ese error al cliente.
        if (!respuestaApi.ok) {
            const errorData = await respuestaApi.json().catch(() => ({}));
            const message = errorData?.error?.message || 'Token inválido o sesión expirada en el backend';
            
            // Limpiamos la cookie rota del navegador
            const respuestaError = NextResponse.json({ error: message }, { status: respuestaApi.status });
            respuestaError.cookies.set('kamples_token', '', { expires: new Date(0), path: '/' });
            return respuestaError;
        }

        // Si el backend confirma que el token es válido, nos dará los datos del usuario.
        const datosUsuario = await respuestaApi.json();

        // NOTA: Tu API puede devolver los datos directamente o anidados en un objeto "data".
        // Asumo que puede ser { "data": { ...usuario } } o simplemente { ...usuario }.
        // Este código contempla ambas posibilidades.
        const usuario = datosUsuario.data || datosUsuario;

        return NextResponse.json({ usuario });

    } catch (error) {
        console.error("Error al verificar la sesión con el backend:", error);
        // Este error ocurriría por un problema de red o un timeout.
        return NextResponse.json({ error: 'Error de comunicación con el servidor de autenticación' }, { status: 500 });
    }
}