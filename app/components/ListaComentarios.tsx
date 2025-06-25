"use client"
import React from 'react';
import { FixedSizeList as List } from 'react-window';

interface Comentario {
    id: number;
    texto: string;
    autor: string;
}

interface ListaComentariosProps {
    comentarios: Comentario[];
}

const ListaComentarios: React.FC<ListaComentariosProps> = ({ comentarios }) => {
    const Row = ({ index, style }: { index: number, style: React.CSSProperties }) => {
        const comentario = comentarios[index];
        return (
            <div style={style} className="border-b p-4">
                <p className="font-bold">{comentario.autor}</p>
                <p>{comentario.texto}</p>
            </div>
        );
    };

    return (
        <List
            height={400}
            itemCount={comentarios.length}
            itemSize={80}
            width={'100%'}
        >
            {Row}
        </List>
    );
};

export default ListaComentarios;