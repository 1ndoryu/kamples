/*
 * GridNotas — Grid principal del piano roll.
 * C310: Canvas para fondo (líneas, colores teclas negras) + DOM divs para notas.
 * Misma estrategia que FL Studio: overlay de notas interactivas sobre grid estático.
 */

import type { CSSProperties } from 'react';
import { useGridNotas } from '../../../hooks/varios-1/useGridNotas';

interface GridNotasProps {
    totalCompases: number;
    numerador?: number;
    notaHover: number | null;
}

export const GridNotas = ({
    totalCompases,
    numerador = 4,
    notaHover,
}: GridNotasProps): JSX.Element => {
    const {
        contenedorRef,
        canvasRef,
        handleGridMouseDown,
        handleWheel,
        cursorGrid,
        marquee,
        altTotal,
        anchoTotal,
        notasRenderizadas,
        hoverStyle,
    } = useGridNotas({ totalCompases, numerador, notaHover });

    return (
        <div
            ref={contenedorRef}
            className="pianoRollGrid"
            style={{ '--cursorGrid': cursorGrid() } as CSSProperties}
            onMouseDown={handleGridMouseDown}
            onWheel={handleWheel}
        >
            {/* Canvas: fondo del grid */}
            <canvas
                ref={canvasRef}
                className="pianoRollGridCanvas"
            />

            {/* Highlight de fila hover */}
            {hoverStyle && (
                <div
                    className="pianoRollFilaHover"
                    style={{
                        '--filaHoverTop': `${hoverStyle.top}px`,
                        '--filaHoverAlto': `${hoverStyle.height}px`,
                    } as CSSProperties}
                />
            )}

            {/* Contenedor de notas DOM */}
            <div
                className="pianoRollNotasContenedor"
                style={{
                    '--notasAncho': `${anchoTotal}px`,
                    '--notasAlto': `${altTotal}px`,
                } as CSSProperties}
            >
                {notasRenderizadas}
            </div>

            {/* Marquee de selección */}
            {marquee && (
                <div
                    className="pianoRollMarquee"
                    style={{
                        '--marqueeIzq': `${marquee.x}px`,
                        '--marqueeTop': `${marquee.y}px`,
                        '--marqueeAncho': `${marquee.ancho}px`,
                        '--marqueeAlto': `${marquee.alto}px`,
                    } as CSSProperties}
                />
            )}
        </div>
    );
};
