// app/login/page.tsx
'use client';

import { useState, useEffect } from 'react';
import Boton from '@/components/ui/Boton';
// import { useAuth } from '@/context/AuthContext'; // Eliminado
import { useAppStore } from '@/store/useAppStore'; // Importar el store de Zustand
import { useRouter } from 'next/navigation';

export default function PaginaLogin() {
    // const { login, usuario, cargando: cargandoAuth } = useAuth(); // Eliminado
    const loginStore = useAppStore((state) => state.loginStore);
    const isAuthenticated = useAppStore((state) => state.isAuthenticated);
    const isLoadingAuth = useAppStore((state) => state.isLoading); // Estado de carga del store
    const authError = useAppStore((state) => state.error); // Errores del store
    const clearAuthError = useAppStore((state) => state.setAuthError); // Para limpiar errores

    const router = useRouter();
    const [nombreUsuario, setNombreUsuario] = useState('');
    const [clave, setClave] = useState('');
    // El error local se puede reemplazar o complementar con authError del store
    const [localError, setLocalError] = useState('');
    const [cargandoSubmit, setCargandoSubmit] = useState(false);

    useEffect(() => {
        // Si el usuario está autenticado, redirigir
        if (isAuthenticated && !isLoadingAuth) {
            router.push('/');
        }
    }, [isAuthenticated, isLoadingAuth, router]);

    const manejarSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLocalError('');
        clearAuthError(null); // Limpiar errores previos del store
        setCargandoSubmit(true);

        const resultado = await loginStore(nombreUsuario, clave);

        if (!resultado.exito) {
            setLocalError(resultado.error || 'Ocurrió un error inesperado.');
        } else {
            // La redirección se maneja en el useEffect al cambiar isAuthenticated
            // o podrías forzarla aquí si es necesario: router.push('/');
        }
        setCargandoSubmit(false);
    };

    // Mientras se determina el estado de autenticación (isLoadingAuth) o si ya está autenticado, no mostrar el form.
    // Esto previene parpadeos y mostrar el login a usuarios ya logueados.
    if (isLoadingAuth || isAuthenticated) {
        return null; // O un componente de carga global/esqueleto
    }

    return (
        <>
            <section className="contenedorLogin">
                <div className="formularioContenedor">
                    <h1>Iniciar Sesión</h1>
                    <p>Accede a tu cuenta para empezar a compartir y descubrir samples.</p>
                    <form onSubmit={manejarSubmit}>
                        <div className="campoFormulario">
                            <label htmlFor="nombreUsuario">Nombre de Usuario</label>
                            <input id="nombreUsuario" type="text" value={nombreUsuario} onChange={e => setNombreUsuario(e.target.value)} required disabled={cargandoSubmit}/>
                        </div>
                        <div className="campoFormulario">
                            <label htmlFor="clave">Contraseña</label>
                            <input id="clave" type="password" value={clave} onChange={e => setClave(e.target.value)} required disabled={cargandoSubmit}/>
                        </div>
                        {/* Mostrar error local o del store */}
                        {(localError || authError) && <p className="mensajeError">{localError || authError}</p>}

                        <Boton type="submit" className="anchoCompleto" disabled={cargandoSubmit || isLoadingAuth}>
                            {cargandoSubmit || isLoadingAuth ? 'Iniciando...' : 'Iniciar Sesión'}
                        </Boton>
                    </form>
                </div>
            </section>

            {/* El CSS se mantiene sin cambios */}
            <style jsx>{`
                .contenedorLogin {
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    padding: 4rem 2rem;
                    flex-grow: 1;
                }
                .formularioContenedor {
                    width: 100%;
                    max-width: 400px;
                    padding: 2.5rem;
                    border: 1px solid var(--color-borde);
                    border-radius: 12px;
                    background-color: var(--color-fondo-secundario);
                }
                h1 {
                    text-align: center;
                    margin-bottom: 0.5rem;
                }
                p {
                    text-align: center;
                    margin-bottom: 2rem;
                    opacity: 0.8;
                }
                .campoFormulario {
                    margin-bottom: 1.5rem;
                }
                .campoFormulario label {
                    display: block;
                    margin-bottom: 0.5rem;
                    font-weight: 600;
                }
                .campoFormulario input {
                    width: 100%;
                    padding: 0.75rem;
                    border-radius: 8px;
                    border: 1px solid var(--color-borde);
                    background-color: var(--color-fondo);
                    color: var(--color-texto);
                    font-size: 1rem;
                    transition: border-color 0.2s;
                }
                .campoFormulario input:focus {
                    outline: none;
                    border-color: var(--color-primario);
                }
                .campoFormulario input:disabled {
                    background-color: var(--color-fondo-secundario);
                    opacity: 0.6;
                }
                .mensajeError {
                    color: #e53e3e;
                    background-color: rgba(229, 62, 62, 0.1);
                    border-radius: 8px;
                    padding: 0.75rem;
                    text-align: center;
                    margin-bottom: 1rem;
                }
                :global(.anchoCompleto) {
                    width: 100%;
                }
            `}</style>
        </>
    );
}