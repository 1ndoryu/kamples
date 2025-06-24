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
	// Aseguramos que no haya espacios extraños antes o después de las etiquetas principales
	return (
		<html lang="es" data-tema="oscuro" suppressHydrationWarning>
			{/* Next.js maneja <head /> aquí. No añadir <head> manualmente a menos que sea necesario con contenido específico. */}
			<body className={sourceSans.className}>
				<StoreAuthInitializer />
				<ThemeApplicator />
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