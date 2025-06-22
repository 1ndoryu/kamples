// app/components/layout/Header.tsx
'use client';

import Link from 'next/link';
import InterruptorTema from './InterruptorTema';
import Boton from '@/components/ui/Boton';
import {useAuth} from '@/context/AuthContext';
import SubirSample from '@/components/SubirSample';

export default function Header() {
    const {usuario, logout, cargando} = useAuth();

    return (
        <header className="cabeceraPrincipal">
            <div className="contenedor">
                {/* El logo se puede mantener o mover al menú lateral si se prefiere */}
                <Link href="/" className="logo">
                    Kamples
                </Link>

                {/* La navegación principal ahora está en el menú lateral */}
                {/* Dejamos aquí los controles de usuario y tema */}
                <div className="controlesCabecera">
                    {cargando ? (
                        <div className="cargandoAuth"></div>
                    ) : usuario ? (
                        <>
                            <SubirSample />
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
                </div>
            </div>
            <style jsx>{`
                .cabeceraPrincipal {
                    padding: 1rem 2rem;
                    background-color: var(--color-fondo-secundario);
                    border-bottom: 1px solid var(--color-borde);
                    transition: background-color 0.3s, border-color 0.3s;
                    position: sticky; /* Hacemos el header pegajoso */
                    top: 0;
                    z-index: 10;
                    width: 100%;
                }
                .contenedor {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    width: 100%;
                }
                .logo {
                    font-size: 1.5rem;
                    font-weight: bold;
                    text-decoration: none;
                    color: var(--color-texto);
                }
                .controlesCabecera {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
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
