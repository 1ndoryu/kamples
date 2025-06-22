// app/components/layout/LayoutStyles.tsx
'use client';

// Este componente no renderiza HTML visible. Su única función es
// inyectar los estilos globales del layout principal en la página.
export default function LayoutStyles() {
    return (
        <style jsx global>{`
            :root {
                --ancho-menu-lateral: 240px;
            }
            .layoutPrincipal {
                display: flex;
                min-height: 100vh;
            }
            .contenidoPagina {
                flex-grow: 1;
                display: flex;
                flex-direction: column;
                /* Prevenimos que el contenido se encoja más allá de su tamaño mínimo */
                min-width: 0;
            }
            .contenidoPrincipal {
                flex-grow: 1;
                padding: 2rem;
            }
            .cargandoContenido {
                padding: 2rem;
            }

            /* Estilos del Header (movidos de Header.tsx para una mejor gestión) */
            .cabeceraPrincipal {
                display: flex;
                align-items: center;
                justify-content: space-between;
                padding: 1rem 2rem;
                border-bottom: 1px solid var(--color-borde);
                background-color: var(--color-fondo);
                flex-shrink: 0; /* Evita que el header se encoja */
            }
            .espaciador {
                flex-grow: 1;
            }
            .controlesCabecera {
                display: flex;
                align-items: center;
                gap: 1rem;
            }
            .nombreUsuario {
                font-weight: 600;
            }
            .cargandoAuth {
                width: 70px;
                height: 24px;
                border-radius: 8px;
                background-color: var(--color-borde);
            }
        `}</style>
    );
}
