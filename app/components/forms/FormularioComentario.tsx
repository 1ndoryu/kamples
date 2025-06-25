"use client"
import React, { useState } from 'react';
import Boton from '../ui/Boton';

interface FormularioComentarioProps {
    onSubmit: (texto: string) => void;
}

const FormularioComentario: React.FC<FormularioComentarioProps> = ({ onSubmit }) => {
    const [texto, setTexto] = useState('');

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();
        if (texto.trim()) {
            onSubmit(texto);
            setTexto('');
        }
    };

    return (
        <form onSubmit={handleSubmit} className="mb-6">
            <textarea
                className="w-full p-2 border rounded mb-2"
                rows={3}
                value={texto}
                onChange={(e) => setTexto(e.target.value)}
                placeholder="Escribe un comentario..."
            ></textarea>
            <Boton type="submit">Enviar Comentario</Boton>
        </form>
    );
};

export default FormularioComentario;