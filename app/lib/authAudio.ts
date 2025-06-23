// app/lib/authAudio.ts
import {SignJWT, jwtVerify} from 'jose';

const secretKey = process.env.AUDIO_TOKEN_SECRET;
const key = new TextEncoder().encode(secretKey);

interface AudioTokenPayload {
    mediaUrl: string;
}

export async function generarTokenAudio(payload: AudioTokenPayload): Promise<string> {
    if (!secretKey) {
        throw new Error('La clave secreta para tokens de audio no está configurada.');
    }

    return new SignJWT(payload)
        .setProtectedHeader({alg: 'HS256'})
        .setIssuedAt()
        .setExpirationTime('10m') // Token de corta duración (10 minutos)
        .sign(key);
}

export async function verificarTokenAudio(token: string): Promise<AudioTokenPayload> {
    if (!secretKey) {
        throw new Error('La clave secreta para tokens de audio no está configurada.');
    }

    const {payload} = await jwtVerify(token, key, {
        algorithms: ['HS256']
    });
    return payload as AudioTokenPayload;
}
