// app/components/layout/Footer.tsx
'use client';

export default function Footer() {
    return (
        <footer className="pieDePagina">
            <p>&copy; {new Date().getFullYear()} Kamples. Todos los derechos reservados.</p>
            <style jsx>{`
                .pieDePagina {
                    padding: 2rem;
                    text-align: center;
                    background-color: var(--color-fondo-secundario); /* Usamos variable */
                    border-top: 1px solid var(--color-borde); /* Usamos variable */
                    margin-top: auto;
                    transition: background-color 0.3s, border-color 0.3s;
                }
            `}</style>
        </footer>
    );
}