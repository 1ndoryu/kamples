// app/components/ui/Modal.tsx
'use client';

import {useEffect} from 'react';

interface Props {
    titulo: string;
    estaAbierto: boolean;
    alCerrar: () => void;
    children: React.ReactNode;
}

export default function Modal({titulo, estaAbierto, alCerrar, children}: Props) {
    useEffect(() => {
        const manejarTeclaEsc = (evento: KeyboardEvent) => {
            if (evento.key === 'Escape') {
                alCerrar();
            }
        };

        if (estaAbierto) {
            document.addEventListener('keydown', manejarTeclaEsc);
        }

        return () => {
            document.removeEventListener('keydown', manejarTeclaEsc);
        };
    }, [estaAbierto, alCerrar]);

    if (!estaAbierto) {
        return null;
    }

    return (
        <>
            <div className="modalTelon" onClick={alCerrar}>
                <div className="modalContenedor" onClick={e => e.stopPropagation()}>
                    <main className="modalCuerpo">{children}</main>
                </div>
            </div>

            <style jsx>{`
                .modalTelon {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    background-color: rgba(0, 0, 0, 0.7);
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    z-index: 1000;
                }
                .modalContenedor {
                    background-color: var(--fondo);
                    border: var(--borde);
                    border-radius: var(--radius);
                    width: 90%;
                    max-width: 600px;
                    max-height: 90vh;
                    display: flex;
                    flex-direction: column;
                    animation: aparecer 0.2s ease-out;
                }
                @keyframes aparecer {
                    from {
                        transform: scale(0.95);
                        opacity: 0;
                    }
                    to {
                        transform: scale(1);
                        opacity: 1;
                    }
                }
                .modalCabecera {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    padding: 1rem 1.5rem;
                    border-bottom: 1px solid var(--color-borde);
                }
                .modalTitulo {
                    margin: 0;
                    font-size: 1rem;
                    font-weight: 600;
                }
                .modalBotonCerrar {
                    background: none;
                    border: none;
                    font-size: 2rem;
                    line-height: 1;
                    cursor: pointer;
                    color: var(--color-texto);
                    opacity: 0.6;
                    padding: 0;
                }
                .modalBotonCerrar:hover {
                    opacity: 1;
                }
                .modalCuerpo {
                    padding: 1.2rem;
                    overflow-y: auto;
                }
            `}</style>
        </>
    );
}
