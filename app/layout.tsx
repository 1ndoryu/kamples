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
                {/* El bloque <style jsx global> ha sido eliminado para corregir el error */}
            </body>
        </html>
    );
}