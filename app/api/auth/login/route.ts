// app/api/auth/login/route.ts
import {NextResponse} from 'next/server';

export async function POST(request: Request) {
	try {
		const {nombreUsuario, clave} = await request.json();

		// CORRECCIÓN 1: Usar la URL completa de la API desde las variables de entorno.
		const apiUrl = process.env.NEXT_PUBLIC_SWORD_API_URL;
		if (!apiUrl) {
			throw new Error('La variable de entorno NEXT_PUBLIC_SWORD_API_URL no está configurada.');
		}

		// 1. Llamamos a tu API de SwordPHP desde nuestro servidor con la URL y el body correctos.
		const respuestaApi = await fetch(`${apiUrl}/auth/token`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			// CORRECCIÓN 2: El campo ahora es 'nombre_usuario' para coincidir con la API.
			body: JSON.stringify({nombre_usuario: nombreUsuario, clave: clave})
		});

		// Si la respuesta no es JSON, podemos obtener una pista del error.
		const contentType = respuestaApi.headers.get('content-type');
		if (!contentType || !contentType.includes('application/json')) {
			const textoRespuesta = await respuestaApi.text();
			console.error('La respuesta de la API de SwordPHP no es JSON:', textoRespuesta);
			// Retornamos un error más descriptivo al cliente.
			return NextResponse.json(
				{
					error: `Error de comunicación con el backend. La ruta ${apiUrl}/auth/token devolvió un error ${respuestaApi.status}.`
				},
				{status: 502} // 502 Bad Gateway es apropiado aquí
			);
		}

		const datos = await respuestaApi.json();

		// 2. Si las credenciales son incorrectas, tu API devuelve un error
		if (!respuestaApi.ok) {
			// Asumimos que el error viene en datos.error.message o similar.
			return NextResponse.json({error: datos.error?.message || 'Error de autenticación'}, {status: respuestaApi.status});
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
		const mensaje = error instanceof Error ? error.message : 'Error desconocido';
		console.error('Error en /api/auth/login:', mensaje);
		return NextResponse.json({error: 'Error interno del servidor'}, {status: 500});
	}
}