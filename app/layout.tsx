// app/layout.tsx
import type {Metadata} from 'next';
import {Source_Sans_3} from 'next/font/google';
import './globals.css';

import Header from '@/components/layout/Header';
import Footer from '@/components/layout/Footer';
import MenuLateral from '@/components/layout/MenuLateral'; // <-- 1. Importar
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
                        {/* --- 2. Modificar la estructura principal --- */}
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

                {/* --- 3. Añadir CSS mínimo para la nueva estructura --- */}
                <style jsx global>{`
                    .layoutPrincipal {
                        display: flex;
                        min-height: 100vh;
                    }
                    .contenidoPagina {
                        flex-grow: 1;
                        display: flex;
                        flex-direction: column;
                    }
                    .contenidoPrincipal {
                       flex-grow: 1;
                       padding: 2rem; /* Añadimos padding aquí para consistencia */
                    }
                `}</style>

            </body>
        </html>
    );
}