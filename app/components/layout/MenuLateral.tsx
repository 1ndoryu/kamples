// app/components/layout/MenuLateral.tsx
'use client';

import Link from 'next/link';
import {usePathname} from 'next/navigation';

export default function MenuLateral() {
	const pathname = usePathname();

	const enlaces = [
		{href: '/', texto: 'Inicio'},
		{href: '/explorar', texto: 'Explorar'}
		// Añadir más enlaces aquí en el futuro (ej. Mis Samples, Perfil)
	];

	return (
		<>
			<aside className="menuLateral">
				<div className="logoContenedor">
					<Link href="/" className="logo">
						Kamples
					</Link>
				</div>
				<nav className="navegacionPrincipal">
					<ul>
						{enlaces.map(enlace => (
							<li key={enlace.href}>
								<Link href={enlace.href} className={`enlaceMenu ${pathname === enlace.href ? 'activo' : ''}`}>
									{enlace.texto}
								</Link>
							</li>
						))}
					</ul>
				</nav>
			</aside>
			<style jsx>{`
				.menuLateral {
					width: var(--ancho-menu-lateral);
					flex-shrink: 0;
					background-color: var(--color-fondo-secundario);
					border-right: 1px solid var(--color-borde);
					padding: 1.5rem;
					display: flex;
					flex-direction: column;
					gap: 2rem;
				}
				.logoContenedor {
					text-align: center;
				}
				.logo {
					font-size: 1.8rem;
					font-weight: 700;
					color: var(--color-texto);
					text-decoration: none;
				}
				.navegacionPrincipal ul {
					list-style: none;
					padding: 0;
					margin: 0;
					display: flex;
					flex-direction: column;
					gap: 0.5rem;
				}
				.enlaceMenu {
					display: block;
					padding: 0.75rem 1rem;
					border-radius: 8px;
					text-decoration: none;
					color: var(--color-texto);
					font-weight: 500;
					transition: background-color 0.2s, color 0.2s;
				}
				.enlaceMenu:hover {
					background-color: var(--color-tarjeta-fondo-hover);
				}
				.enlaceMenu.activo {
					background-color: var(--color-primario);
					color: var(--color-texto-primario);
					font-weight: 600;
				}
			`}</style>
		</>
	);
}