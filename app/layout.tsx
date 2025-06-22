// app/layout.tsx
import type {Metadata} from 'next';
import {Source_Sans_3} from 'next/font/google';
import './globals.css';

import Header from '@/components/layout/Header';
import Footer from '@/components/layout/Footer';
import MenuLateral from '@/components/layout/MenuLateral';
import {TemaProvider} from '@/context/TemaContext';
import {AuthProvider} from '@/context/AuthContext';

const sourceSans = Source_Sans_3({
	variable: '--font-principal',
	subsets: ['latin'],
	display: 'swap'
});

export const metadata: Metadata = {
	title: 'Kamples - Tu biblioteca de samples',
	description: 'Encuentra, comparte y colabora con miles de samples de alta calidad.'
};

export default function RootLayout({
	children
}: Readonly<{
	children: React.ReactNode;
}>) {
	return (
		<html lang="es" data-tema="oscuro" suppressHydrationWarning>
			<body className={sourceSans.className}>
				<TemaProvider>
					<AuthProvider>
						<div className="layoutPrincipal">
							<MenuLateral />
							<div className="contenidoPagina">
								<Header />
								<main className="contenidoPrincipal">{children}</main>
								<Footer />
							</div>
						</div>
					</AuthProvider>
				</TemaProvider>
				{/* (CORRECCIÓN) Estilos globales para la estructura principal del layout */}
				<style jsx global>{`
					:root {
						--ancho-menu-lateral: 240px;
					}
					.layoutPrincipal {
						display: flex;
						min-height: 100vh;
					}
					.contenidoPagina {
						flex-grow: 1;
						display: flex;
						flex-direction: column;
						width: calc(100% - var(--ancho-menu-lateral));
					}
					.contenidoPrincipal {
						flex-grow: 1;
						padding: 2rem;
					}
					.cargandoContenido {
						padding: 2rem;
					}
				`}</style>
			</body>
		</html>
	);
}