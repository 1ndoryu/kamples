// app/lib/authAudio.ts
import { SignJWT, jwtVerify, type JWTPayload } from 'jose';

const secretKey = process.env.AUDIO_TOKEN_SECRET;
if (!secretKey) {
	throw new Error('La clave secreta para tokens de audio (AUDIO_TOKEN_SECRET) no está configurada en .env.local');
}
const key = new TextEncoder().encode(secretKey);


// --- CORRECCIÓN 1 ---
// Hacemos que nuestra interfaz extienda la interfaz JWTPayload de la librería.
// Esto soluciona el error al pasar el payload al constructor de SignJWT.
interface AudioTokenPayload extends JWTPayload {
	mediaUrl: string;
}

export async function generarTokenAudio(payload: AudioTokenPayload): Promise<string> {
	return new SignJWT(payload)
		.setProtectedHeader({ alg: 'HS256' })
		.setIssuedAt()
		.setExpirationTime('10m') // Token de corta duración (10 minutos)
		.sign(key);
}

export async function verificarTokenAudio(token: string): Promise<AudioTokenPayload> {
	const { payload } = await jwtVerify(token, key, {
		algorithms: ['HS256']
	});
    
    // --- CORRECCIÓN 2 ---
    // Usamos una conversión de tipo explícita a 'unknown' primero.
    // Esto le dice a TypeScript: "Confía en mí, sé que el payload recibido
    // tendrá la forma de AudioTokenPayload".
	return payload as unknown as AudioTokenPayload;
}