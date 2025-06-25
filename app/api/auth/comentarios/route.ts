import { NextResponse } from 'next/server';

let comentarios = [
    { id: 1, sampleId: 1, texto: 'Este es un comentario de prueba', autor: 'Usuario1' },
    { id: 2, sampleId: 1, texto: 'Este es otro comentario', autor: 'Usuario2' },
];

export async function GET(request: Request) {
    const { searchParams } = new URL(request.url);
    const sampleId = searchParams.get('sampleId');
    const filteredComentarios = comentarios.filter(c => c.sampleId === Number(sampleId));
    return NextResponse.json(filteredComentarios);
}

export async function POST(request: Request) {
    const { sampleId, texto, autor } = await request.json();
    const nuevoComentario = {
        id: comentarios.length + 1,
        sampleId,
        texto,
        autor,
    };
    comentarios.push(nuevoComentario);
    return NextResponse.json(nuevoComentario, { status: 201 });
}