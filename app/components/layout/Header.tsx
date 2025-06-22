// app/components/layout/Header.tsx
'use client';

import Link from 'next/link';
import InterruptorTema from './InterruptorTema';
import Boton from '@/components/ui/Boton';
import {useAuth} from '@/context/AuthContext'; // <-- Importamos el hook de autenticación

export default function Header() {
    const {usuario, logout, cargando} = useAuth(); // <-- Obtenemos el usuario y la función logout

    return (
        <header className="cabeceraPrincipal">
            <div className="contenedor">
                <Link href="/" className="logo">
                    Kamples
                </Link>
                <nav className="navegacionPrincipal">
                    <Link href="/explorar">Explorar</Link>

                    {/* Renderizado condicional basado en el estado de carga y el usuario */}
                    {cargando ? (
                        <div className="cargandoAuth"></div> // Placeholder de carga
                    ) : usuario ? (
                        <>
                            <Boton href="/subir" variante="secundario">
                                Subir Sample
                            </Boton>
                            <span className="nombreUsuario">Hola, {usuario.nombremostrado}</span>
                            <Boton onClick={logout} variante="primario">
                                Cerrar Sesión
                            </Boton>
                        </>
                    ) : (
                        <>
                            <Boton href="/login" variante="secundario">
                                Iniciar Sesión
                            </Boton>
                            <Boton href="/registro" variante="primario">
                                Registrarse
                            </Boton>
                        </>
                    )}

                    <InterruptorTema />
                </nav>
            </div>
            <style jsx>{`
                /* ... se mantiene el CSS anterior ... */
                .cabeceraPrincipal {
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
                    align-items: center;
                    gap: 1rem;
                }
                .navegacionPrincipal > :global(a) {
                    text-decoration: none;
                    color: var(--color-texto);
                    opacity: 0.8;
                    transition: opacity 0.2s;
                    padding: 0.6rem 0.4rem;
                }
                .navegacionPrincipal > :global(a):hover {
                    opacity: 1;
                }

                .nombreUsuario {
                    font-weight: 600;
                    opacity: 0.9;
                }

                .cargandoAuth {
                    width: 70px;
                    height: 20px;
                    border-radius: 4px;
                    background-color: var(--color-borde);
                }
            `}</style>
        </header>
    );
}
