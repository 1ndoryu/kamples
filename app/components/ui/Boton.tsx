// app/components/ui/Boton.tsx
'use client';

import Link from 'next/link';

interface Props {
    href?: string;
    onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void; // Tipado correcto para el evento
    children: React.ReactNode;
    variante?: 'primario' | 'secundario';
    className?: string;
    type?: 'button' | 'submit' | 'reset'; // Añadimos el tipo de botón
    disabled?: boolean;
}

export default function Boton({ href, onClick, children, variante = 'primario', className = '', type = 'button', disabled = false }: Props) {
    const clasesBoton = `boton ${variante} ${className}`;

    if (href) {
        return (
            <Link href={href} className={clasesBoton}>
                {children}
            </Link>
        );
    }

    return (
        <>
            <button onClick={onClick} className={clasesBoton} type={type} disabled={disabled}>
                {children}
            </button>

            {/* CORRECCIÓN: Se elimina 'global' para encapsular los estilos (Regla #3) */}
            <style jsx>{`
                .boton {
                    display: inline-block;
                    padding: 0.6rem 1.2rem;
                    border-radius: 8px;
                    border: 1px solid transparent;
                    font-weight: 600;
                    font-size: 0.9rem;
                    text-decoration: none;
                    cursor: pointer;
                    transition: background-color 0.2s, border-color 0.2s, color 0.2s, opacity 0.2s;
                    text-align: center;
                }

                .boton:disabled {
                    opacity: 0.5;
                    cursor: not-allowed;
                }

                /* Variante Primaria */
                .boton.primario {
                    background-color: var(--color-primario);
                    color: var(--color-texto-primario);
                }
                .boton.primario:hover:not(:disabled) {
                    background-color: var(--color-primario-hover);
                }

                /* Variante Secundaria */
                .boton.secundario {
                    background-color: transparent;
                    color: var(--color-texto);
                    border-color: var(--color-borde);
                }
                .boton.secundario:hover:not(:disabled) {
                    background-color: var(--color-tarjeta-fondo-hover);
                    border-color: var(--color-primario);
                }
            `}</style>
        </>
    );
}