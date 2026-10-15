/*
 * MinimapaDaw — Minimapa estilo FL Studio para controlar scroll y zoom.
 * Componente de vista. Lógica en useMinimapaDaw.
 */

import { useMinimapaDaw } from '../../hooks/minimapa/useMinimapaDaw';

interface MinimapaDawProps {
    timelineRef: React.RefObject<HTMLDivElement>;
}

export const MinimapaDaw = ({ timelineRef }: MinimapaDawProps): JSX.Element => {
    const {
        minimapaRef, viewportRef, viewportFrac, viewportLeft,
        bloquesSimplificados, compasesVisibles,
        handleViewportMouseDown, alClickMinimapa, alWheel,
    } = useMinimapaDaw(timelineRef);

    return (
        <div
            ref={minimapaRef}
            className="minimapaDaw"
            onClick={alClickMinimapa}
            onWheel={alWheel}
        >
            {bloquesSimplificados.map((b) => (
                <div
                    key={`${b.left}-${b.top}`}
                    className="minimapaBloque"
                    style={{
                        '--bloqueIzquierda': `${b.left}%`,
                        '--bloqueAncho': `${b.width}%`,
                        '--bloqueArriba': `${b.top}%`,
                        '--bloqueAlto': `${b.height}%`,
                        '--bloqueColor': b.color,
                    } as React.CSSProperties}
                />
            ))}

            <div
                ref={viewportRef}
                className="minimapaViewport"
                style={{
                    '--viewportIzquierda': `${viewportLeft * 100}%`,
                    '--viewportAncho': `${viewportFrac * 100}%`,
                } as React.CSSProperties}
                onMouseDown={handleViewportMouseDown}
            >
                <div className="minimapaHandleIzq" />
                <div className="minimapaHandleDer" />
            </div>

            <span className="minimapaZoomInfo">
                {compasesVisibles < 1
                    ? `${compasesVisibles.toFixed(1)} comp.`
                    : `${Math.round(compasesVisibles)} comp.`
                }
            </span>
        </div>
    );
};
