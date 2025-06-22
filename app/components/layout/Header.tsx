// app/components/layout/Header.tsx
'use client';

import Link from 'next/link';
import InterruptorTema from './InterruptorTema'; // <-- RUTA CORREGIDA

export default function Header() {
    return (
        <header className="cabeceraPrincipal">
            <div className="contenedor">
                <Link href="/" className="logo">
                    Kamples
                </Link>
                <nav className="navegacionPrincipal">
                    <Link href="/explorar">Explorar</Link>
                    <Link href="/subir">Subir Sample</Link>
                    <Link href="/login">Iniciar Sesión</Link>
                    <InterruptorTema />
                </nav>
            </div>
            <style jsx>{`
                .cabeceraPrincipal {
                    /* Usamos variables CSS */
                    padding: 1rem 2rem;
                    background-color: var(--color-fondo-secundario);
                    border-bottom: 1px solid var(--color-borde);
                    transition: background-color 0.3s, border-color 0.3s;
                }
                .contenedor {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    max-width: 1200px;
                    margin: 0 auto;
                }
                .logo {
                    font-size: 1.5rem;
                    font-weight: bold;
                    text-decoration: none;
                    color: var(--color-texto);
                }
                .navegacionPrincipal {
                    display: flex;
                    align-items: center; /* Para alinear el botón con los links */
                    gap: 1.5rem;
                }
                .navegacionPrincipal a {
                    text-decoration: none;
                    color: var(--color-texto);
                    opacity: 0.8;
                    transition: opacity 0.2s;
                }
                .navegacionPrincipal a:hover {
                    opacity: 1;
                }
            `}</style>
        </header>
    );
}