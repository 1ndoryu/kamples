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
        </header>
    );
}
