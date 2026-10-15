/*
 * GhostNotas — Notas fantasma de otros canales del mismo patrón.
 * C310: Se renderizan como divs semitransparentes (sin interacción)
 * dentro del grid del piano roll. Ayudan como referencia visual
 * al componer melodías que complementen a otros canales.
 */

import { memo } from 'react';
import type { CSSProperties } from 'react';
import { useGhostNotas } from '../../../hooks/varios-1/useGhostNotas';

interface GhostNotasProps {
    totalCompases: number;
    numerador?: number;
}

/*
 * Renderiza notas de TODOS los canales del patrón actual excepto
 * el canal activo. Son puramente visuales (pointer-events: none).
 */
export const GhostNotas = memo(({
    totalCompases,
    numerador = 4,
}: GhostNotasProps): JSX.Element | null => {
    const { ghostHabilitado, ghostElements, altTotal, anchoTotal } = useGhostNotas({
        totalCompases,
        numerador,
    });

    if (!ghostHabilitado || ghostElements.length === 0) return null;

    return (
        <div
            className="pianoRollGhostContenedor"
            style={{
                '--ghostAncho': `${anchoTotal}px`,
                '--ghostAlto': `${altTotal}px`,
            } as CSSProperties}
        >
            {ghostElements}
        </div>
    );
});

GhostNotas.displayName = 'GhostNotas';
