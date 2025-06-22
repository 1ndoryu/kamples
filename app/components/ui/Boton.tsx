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

export default function Boton({href, onClick, children, variante = 'primario', className = '', type = 'button', disabled = false}: Props) {
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
                    cursor: pointer;
                    text-align: center;
                    border: 1px solid #0000;
                    border-radius: var(--radius);
                    padding: 0.4rem 1rem;
                    font-size: 12px;
                    text-decoration: none;
                    transition: background-color 0.2s, border-color 0.2s, color 0.2s, opacity 0.2s;
                    display: inline-block;
                }

                .boton:disabled {
                    opacity: 0.5;
                    cursor: not-allowed;
                }

                /* Variante Primaria */
                .boton.primario {
                    background-color: var(--blanco);
                    color: var(--color-texto-primario);
                }
                .boton.primario:hover:not(:disabled) {
                    background-color: var(--color-primario-hover);
                }

                /* Variante Secundaria */
                .boton.secundario {
                    color: var(--color-texto);
                    border: var(--borde);
                    background-color: #0000;
                }
                .boton.secundario:hover:not(:disabled) {
                    background-color: var(--color-tarjeta-fondo-hover);
                    border-color: var(--color-primario);
                }
            `}</style>
        </>
    );
}
