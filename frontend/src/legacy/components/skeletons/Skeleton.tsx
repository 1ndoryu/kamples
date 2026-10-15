/*
 * Componente: Skeleton
 * Bloque de placeholder con animacion pulse.
 * Props: variante (rectangulo, circulo, texto), ancho, alto, className.
 */

import type { CSSProperties } from 'react';
import '../../styles/componentes/varios-6/skeleton.css';

interface SkeletonProps {
    ancho?: string | number;
    alto?: string | number;
    circulo?: boolean;
    className?: string;
}

export function Skeleton({
    ancho,
    alto,
    circulo = false,
    className = '',
}: SkeletonProps): JSX.Element {
    return (
        <div
            className={`skeleton ${circulo ? 'skeletonCirculo' : ''} ${className}`.trim()}
            style={{
                '--ancho': ancho ? (typeof ancho === 'number' ? `${ancho}px` : ancho) : undefined,
                '--alto': alto ? (typeof alto === 'number' ? `${alto}px` : alto) : undefined,
            } as CSSProperties}
            aria-hidden="true"
        />
    );
}
