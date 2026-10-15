/*
 * CursorReproduccion — Línea vertical que muestra la posición actual
 * Se mueve suavemente durante la reproducción, clickeable para seek
 */

import type { CSSProperties } from 'react';
import { useMezcladorStore } from '../../stores/varios-1/mezcladorStore';
import { segundosACompases } from '../../utils/compasUtils';

export const CursorReproduccion = (): JSX.Element => {
    const tiempoActual = useMezcladorStore(s => s.tiempoActual);
    const bpmProyecto = useMezcladorStore(s => s.bpmProyecto);
    const compasProyecto = useMezcladorStore(s => s.compasProyecto);
    /* C285: Usar totalExtendido para alinear con la timeline */
    const totalCompases = useMezcladorStore(s => s.obtenerTotalExtendido());

    const posicionCompases = segundosACompases(tiempoActual, bpmProyecto, compasProyecto);
    /*
     * C222: Calcular left alineado al área de contenido (después de 80px de controles).
     * Los bloques usan left:X% relativo a mezcladorPistaContenido (ancho total - 80px).
     * El cursor debe alinearse al mismo espacio.
     */
    const fraccion = posicionCompases / totalCompases;

    if (fraccion < 0 || fraccion > 1) return <></>;

    return (
        <div
            className="mezcladorCursorReproduccion"
            style={{ '--cursorFraccion': fraccion } as CSSProperties}
        >
            <div className="mezcladorCursorLinea" />
            <div className="mezcladorCursorCabeza" />
        </div>
    );
};
