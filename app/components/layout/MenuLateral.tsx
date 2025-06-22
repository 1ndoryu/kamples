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
        </>
    );
}
