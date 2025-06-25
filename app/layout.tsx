'use client';

import { useEffect } from 'react';
import { useAuthStore } from '@/store/auth';


import {Source_Sans_3} from 'next/font/google';
import './globals.css';

import Header from '@/components/layout/Header';
import MenuLateral from '@/components/layout/MenuLateral';
import {TemaProvider} from '@/context/TemaContext';
import LayoutStyles from '@/components/layout/LayoutStyles';

const sourceSans = Source_Sans_3({
	variable: '--font-principal',
	subsets: ['latin'],
	display: 'swap'
});



export default function RootLayout({
	children
}: Readonly<{
	children: React.ReactNode;
}>) {
    const { verificarSesion } = useAuthStore();

    useEffect(() => {
        verificarSesion();
    }, [verificarSesion]);

	return (
		<html lang="es" data-tema="oscuro" suppressHydrationWarning>
			<body className={sourceSans.className}>
				<TemaProvider>
						<LayoutStyles />
						<div className="layoutPrincipal">
							<MenuLateral />
							<div className="contenidoPagina">
								<Header />
								<main className="contenidoPrincipal">{children}</main>
							</div>
						</div>
				</TemaProvider>
			</body>
		</html>
	);
}