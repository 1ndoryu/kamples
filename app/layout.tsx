// app/layout.tsx
import type { Metadata } from 'next';
import { Geist, Geist_Mono } from 'next/font/google';
import './globals.css';

import Header from '@/components/layout/Header';
import Footer from '@/components/layout/Footer';
import { TemaProvider } from '@/context/TemaContext';

const geistSans = Geist({
    variable: '--font-geist-sans',
    subsets: ['latin']
});

const geistMono = Geist_Mono({
    variable: '--font-geist-mono',
    subsets: ['latin']
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
            <body className={`${geistSans.variable} ${geistMono.variable}`}>
                <TemaProvider>
                    <div className="appContenedor">
                        <Header />
                        <main>{children}</main>
                        <Footer />
                    </div>
                </TemaProvider>
            </body>
        </html>
    );
}