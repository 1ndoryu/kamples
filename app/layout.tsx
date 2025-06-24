// app/layout.tsx
import type {Metadata} from 'next';
import {Source_Sans_3} from 'next/font/google';
import './globals.css';

import Header from '@/components/layout/Header';
import MenuLateral from '@/components/layout/MenuLateral';
// import {TemaProvider} from '@/context/TemaContext'; // Eliminado
import { StoreAuthInitializer } from '@/components/utils/StoreAuthInitializer';
import { ThemeApplicator } from '@/components/utils/ThemeApplicator'; // Nuevo aplicador de tema
import LayoutStyles from '@/components/layout/LayoutStyles';

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
		<html lang="es" data-tema="oscuro" suppressHydrationWarning> {/* `data-tema` aquí es el valor inicial del lado del servidor, ThemeApplicator lo sincronizará en el cliente */}
			<body className={sourceSans.className}>
				{/* TemaProvider eliminado */}
				<StoreAuthInitializer />
				<ThemeApplicator /> {/* Añadido el aplicador de tema */}
				<LayoutStyles />
				<div className="layoutPrincipal">
					<MenuLateral />
					<div className="contenidoPagina">
						<Header />
						<main className="contenidoPrincipal">{children}</main>
					</div>
				</div>
			</body>
		</html>
	);
}