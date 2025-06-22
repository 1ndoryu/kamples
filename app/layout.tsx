// app/layout.tsx
import type {Metadata} from 'next';
import {Source_Sans_3} from 'next/font/google';
import './globals.css';

import Header from '@/components/layout/Header';
import Footer from '@/components/layout/Footer';
import {TemaProvider} from '@/context/TemaContext';
import {AuthProvider} from '@/context/AuthContext'; // <-- 1. Importar

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
                        {' '}
                        {/* <-- 2. Envolver la aplicación */}
                        <div className="appContenedor">
                            <Header />
                            <main>{children}</main>
                            <Footer />
                        </div>
                    </AuthProvider>
                </TemaProvider>
            </body>
        </html>
    );
}
