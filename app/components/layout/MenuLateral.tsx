// app/components/layout/MenuLateral.tsx
'use client';

import Link from 'next/link';
import {usePathname} from 'next/navigation';

export default function MenuLateral() {
	const pathname = usePathname();

	const enlaces = [
		{href: '/', texto: ''},

	];

	return (
		<>
			<aside className="menuLateral">
				<div className="logoContenedor">
					<Link href="/" className="logo">
						<svg id="uuid-aaaa7a22-0b65-42e8-a69c-056d1307c32f" data-name="Capa 1" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 25.2 25.2">
							<rect y="12.6" width="12.6" height="12.6" fill="#efefef" />
							<rect x="12.6" width="12.6" height="12.6" fill="#efefef" />
						</svg>
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
		</>
	);
}