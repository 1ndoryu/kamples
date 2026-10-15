/*
 * EQVisualizer — Visualización de curva de EQ para un insert del mixer.
 * Componente de vista. Lógica en useEQVisualizer.
 */

import type { CSSProperties } from 'react';
import { useEQVisualizer, COLORES_BANDA, NOMBRES_BANDA } from '../../hooks/varios-1/useEQVisualizer';
import type { BandaEQ } from '../../types/mezclador';
import { BotonBase } from '@app/components/ui/boton/BotonBase';

interface EQVisualizerProps {
    bandas: BandaEQ[];
    activo: boolean;
    onToggle: () => void;
    onCambioBanda: (indice: number, cambios: Partial<BandaEQ>) => void;
    ancho?: number;
    alto?: number;
}

export const EQVisualizer = ({
    bandas,
    activo,
    onToggle,
    onCambioBanda,
    ancho = 240,
    alto = 100,
}: EQVisualizerProps): JSX.Element => {
    const { canvasRef, alMouseDown, alMouseMove, alMouseUp } = useEQVisualizer({
        bandas, activo, onCambioBanda, ancho, alto,
    });

    /*
     * [15A-7] La paleta vive en COLORES_BANDA porque el canvas la necesita como
     * literal. Aqui solo se publica como custom properties para que el CSS pinte
     * las etiquetas de cada banda sin duplicar los colores.
     */
    return (
        <div
            className="eqVisualizer"
            style={{
                '--eqBanda1': COLORES_BANDA[0],
                '--eqBanda2': COLORES_BANDA[1],
                '--eqBanda3': COLORES_BANDA[2],
            } as CSSProperties}
        >
            <div className="eqVisualizerCabecera">
                <span>EQ</span>
                <BotonBase variante="ghost"
                    className={`eqVisualizerToggle ${activo ? 'eqVisualizerToggleActivo' : ''}`}
                    onClick={onToggle}
                >
                    {activo ? 'ON' : 'OFF'}
                </BotonBase>
            </div>
            <canvas
                ref={canvasRef}
                width={ancho}
                height={alto}
                className="eqVisualizerCanvas"
                onMouseDown={alMouseDown}
                onMouseMove={alMouseMove}
                onMouseUp={alMouseUp}
                onMouseLeave={alMouseUp}
            />
            <div className="eqVisualizerBandas">
                {bandas.map((banda, i) => (
                    <div key={NOMBRES_BANDA[i]} className="eqVisualizerBanda">
                        <span className="eqVisualizerBandaNombre">{NOMBRES_BANDA[i]}</span>
                        <span className="eqVisualizerBandaValor">{banda.frecuencia}Hz</span>
                        <span className="eqVisualizerBandaValor">{banda.ganancia > 0 ? '+' : ''}{banda.ganancia}dB</span>
                    </div>
                ))}
            </div>
        </div>
    );
};
