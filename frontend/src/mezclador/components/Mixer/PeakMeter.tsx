/*
 * PeakMeter — Barra vertical L/R con indicadores de nivel.
 * Colores: verde → amarillo → rojo según nivel.
 * Animación suave con CSS transitions.
 */

import { memo } from 'react';
import type { CSSProperties } from 'react';

interface PeakMeterProps {
    peakL: number;
    peakR: number;
    alto?: number;
}

/* Nivel (0-1) → token de la escala del medidor, definida en mezcladorMixer.css */
const nivelAToken = (nivel: number): string => {
    if (nivel > 0.9) return 'var(--medidorSaturado)';
    if (nivel > 0.7) return 'var(--medidorAlto)';
    if (nivel > 0.5) return 'var(--medidorMedio)';
    return 'var(--medidorBajo)';
};

export const PeakMeter = memo(({
    peakL,
    peakR,
    alto = 100,
}: PeakMeterProps): JSX.Element => {
    const alturaL = Math.min(100, peakL * 100);
    const alturaR = Math.min(100, peakR * 100);

    return (
        <div className="peakMeter" style={{ '--altoMedidor': `${alto}px` } as CSSProperties}>
            {/* Canal izquierdo */}
            <div className="peakMeterCanal">
                <div
                    className="peakMeterBarra"
                    style={{
                        '--nivelBarra': `${alturaL}%`,
                        '--colorBarra': nivelAToken(peakL),
                    } as CSSProperties}
                />
            </div>

            {/* Canal derecho */}
            <div className="peakMeterCanal">
                <div
                    className="peakMeterBarra"
                    style={{
                        '--nivelBarra': `${alturaR}%`,
                        '--colorBarra': nivelAToken(peakR),
                    } as CSSProperties}
                />
            </div>
        </div>
    );
});

PeakMeter.displayName = 'PeakMeter';
