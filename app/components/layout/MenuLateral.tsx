// app/components/layout/MenuLateral.tsx
'use client';

import Link from 'next/link';
import {usePathname} from 'next/navigation';

// Iconos simples como componentes para mantener el código limpio
const IconoInicio = () => <svg /* ... */>...</svg>;
const IconoExplorar = () => <svg /* ... */>...</svg>;

export default function MenuLateral() {
    const pathname = usePathname();

    const enlaces = [
        {href: '/', texto: 'Inicio', icono: <IconoInicio />},
        {href: '/explorar', texto: 'Explorar', icono: <IconoExplorar />}
        // Añadir más enlaces aquí en el futuro (ej. Mis Samples, Perfil)
    ];

    return (
        <>
            <aside className="menuLateral">
                <nav>
                    <ul>
                        {enlaces.map(enlace => (
                            <li key={enlace.href}>
                                <Link href={enlace.href} className={`enlaceMenu ${pathname === enlace.href ? 'activo' : ''}`}>
                                    {/* {enlace.icono} */}
                                    <span className="textoEnlace">{enlace.texto}</span>
                                </Link>
                            </li>
                        ))}
                    </ul>
                </nav>
            </aside>
            <style jsx>{`
                .menuLateral {
                    width: 240px;
                    background-color: var(--color-fondo-secundario);
                    border-right: 1px solid var(--color-borde);
                    padding: 2rem 1rem;
                    display: flex;
                    flex-direction: column;
                    transition: background-color 0.3s, border-color 0.3s;
                }
                nav ul {
                    list-style: none;
                    padding: 0;
                    margin: 0;
                }
                .enlaceMenu {
                    display: flex;
                    align-items: center;
                    padding: 0.75rem 1rem;
                    border-radius: 8px;
                    text-decoration: none;
                    color: var(--color-texto);
                    font-weight: 500;
                    margin-bottom: 0.5rem;
                    transition: background-color 0.2s, color 0.2s;
                }
                .enlaceMenu:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                }
                .enlaceMenu.activo {
                    background-color: var(--color-primario);
                    color: var(--color-texto-primario);
                }
                .textoEnlace {
                    margin-left: 0.75rem;
                }
            `}</style>
        </>
    );
}
