/*
 * Error Boundary individual por isla.
 * Captura errores de renderizado sin que una isla rota tumbe las demas.
 * En desarrollo muestra detalles del error; en produccion, un fallback limpio.
 */

import { Component, type ReactNode, type ErrorInfo } from 'react';
import { logger } from "../utils/logger";

interface ErrorBoundaryProps {
    islandName: string;
    fallback?: ReactNode;
    /** [193A-46] Cambiar este valor resetea el error boundary automáticamente.
     * Útil para keep-alive SPA: al re-navegar a una isla, el orden cambia
     * y el boundary se auto-limpia sin remontaje. */
    resetKey?: number;
    children: ReactNode;
}

interface ErrorBoundaryState {
    hasError: boolean;
    error: Error | null;
}

export class IslandErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
    state: ErrorBoundaryState = { hasError: false, error: null };

    static getDerivedStateFromError(error: Error): ErrorBoundaryState {
        return { hasError: true, error };
    }

    componentDidCatch(error: Error, info: ErrorInfo): void {
        logger.error(
            `[Glory] Error en isla "${this.props.islandName}":`,
            error,
            info.componentStack,
        );
    }

    /* [193A-46] Auto-reset cuando cambia resetKey (re-navegación SPA keep-alive) */
    componentDidUpdate(prevProps: ErrorBoundaryProps): void {
        if (
            this.state.hasError &&
            this.props.resetKey !== undefined &&
            prevProps.resetKey !== this.props.resetKey
        ) {
            this.setState({ hasError: false, error: null });
        }
    }

    render(): ReactNode {
        if (!this.state.hasError) {
            return this.props.children;
        }

        /* Fallback personalizado del usuario */
        if (this.props.fallback) {
            return this.props.fallback;
        }

        /* En desarrollo: mostrar detalles del error */
        if (import.meta.env.DEV) {
            return (
                <div className="islaError">
                    <strong>Error en isla &quot;{this.props.islandName}&quot;</strong>
                    <pre className="islaErrorDetalle">
                        {this.state.error?.message}
                    </pre>
                    <button
                        className="islaErrorReintentar"
                        onClick={() => this.setState({ hasError: false, error: null })}
                    >
                        Reintentar
                    </button>
                </div>
            );
        }

        /* En produccion: fallback limpio con boton reintentar */
        return (
            <div className="islaErrorVacio">
                Contenido no disponible
                <br />
                <button
                    className="islaErrorVacioReintentar"
                    onClick={() => this.setState({ hasError: false, error: null })}
                >
                    Reintentar
                </button>
            </div>
        );
    }
}
