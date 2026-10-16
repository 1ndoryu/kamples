/*
 * DevOverlay — Overlay de debug visible solo en desarrollo.
 * Muestra el nombre de la isla, conteo de renders y resumen de props.
 * Se activa automaticamente con import.meta.env.DEV.
 */

import type { ReactNode } from 'react';

interface DevOverlayProps {
    islandName: string;
    props: Record<string, unknown>;
    children: ReactNode;
}

export function DevOverlay({ islandName, props, children }: DevOverlayProps): JSX.Element {
    const propsKeys = Object.keys(props);
    const propsResumen = propsKeys.length > 0 ? propsKeys.join(', ') : 'sin props';

    return (
        <div className="devOverlay">
            <div
                className="devOverlayEtiqueta"
                title={`Props: ${propsResumen}`}
            >
                {islandName}
            </div>
            {children}
        </div>
    );
}
