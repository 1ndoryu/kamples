/*
 * ComentarioPreview — QQ20
 * Muestra el comentario con más likes como preview compacto debajo de la barra de acciones.
 * Clickeable para abrir la sección completa de comentarios.
 */

import { ThumbsUp } from 'lucide-react';
import { Avatar } from '@app/components/ui/varios-1/Avatar';
import { formatearTiempoRelativo } from '@app/utils/varios-2/tiempo';
import type { ComentarioDestacado } from '@app/types';
import '../../../styles/componentes/varios-2/comentarioPreview.css';

interface ComentarioPreviewProps {
    comentario: ComentarioDestacado;
    onClick?: () => void;
}

export const ComentarioPreview = ({ comentario, onClick }: ComentarioPreviewProps): JSX.Element => {
    return (
        <div
            className="comentarioPrevContenedor"
            onClick={onClick}
            role={onClick ? 'button' : undefined}
            tabIndex={onClick ? 0 : undefined}
            onKeyDown={onClick ? (e) => { if (e.key === 'Enter') onClick(); } : undefined}
        >
            <Avatar
                src={comentario.autor.avatarUrl}
                nombre={comentario.autor.nombreVisible}
                tamano="xs"
            />
            <div className="comentarioPrevCuerpo">
                <div className="comentarioPrevCabecera">
                    <span className="comentarioPrevAutor">{comentario.autor.nombreVisible}</span>
                    <span className="comentarioPrevTiempo">{formatearTiempoRelativo(comentario.creadoAt)}</span>
                </div>
                <p className="comentarioPrevTexto">{comentario.contenido}</p>
            </div>
            {comentario.totalLikes > 0 && (
                <div className="comentarioPrevLikes">
                    <ThumbsUp size={12} />
                    <span>{comentario.totalLikes}</span>
                </div>
            )}
        </div>
    );
};
