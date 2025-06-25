"use client"
import React, { useState, useEffect } from 'react';
import FormularioComentario from './forms/FormularioComentario';
import ListaComentarios from './ListaComentarios';
import { obtenerComentarios, crearComentario } from '@/services/swordApi';

interface Comentario {
    id: number;
    texto: string;
    autor: string;
}

interface ComentariosProps {
    sampleId: number;
}

const Comentarios: React.FC<ComentariosProps> = ({ sampleId }) => {
    const [comentarios, setComentarios] = useState<Comentario[]>([]);

    useEffect(() => {
        const fetchComentarios = async () => {
            const comentariosObtenidos = await obtenerComentarios(sampleId);
            setComentarios(comentariosObtenidos);
        };
        fetchComentarios();
    }, [sampleId]);

    const handleNuevoComentario = async (texto: string) => {
        const nuevoComentario = await crearComentario(sampleId, texto);
        setComentarios(prevComentarios => [...prevComentarios, nuevoComentario]);
    };

    return (
        <div className="container mx-auto mt-8">
            <h2 className="text-2xl font-bold mb-4">Comentarios</h2>
            <FormularioComentario onSubmit={handleNuevoComentario} />
            <ListaComentarios comentarios={comentarios} />
        </div>
    );
};

export default Comentarios;