'use client';
import { useState, useEffect } from 'react';
import Boton from '@/components/ui/Boton';
import { useAuth } from '@/context/AuthContext';
import { useRouter } from 'next/navigation';

export default function PaginaLogin() {
    const { login, usuario, cargando: cargandoAuth } = useAuth();
    const router = useRouter();
    const [credenciales, setCredenciales] = useState({
        nombreUsuario: '',
        clave: ''
    })
    const [error, setError] = useState('');
    const [cargandoSubmit, setCargandoSubmit] = useState(false);

    useEffect(() => {
        if (usuario && !cargandoAuth) {
            router.push('/');
        }
    }, [usuario, cargandoAuth, router]);

    const manejarSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError('');
        setCargandoSubmit(true);

        const resultado = await login(credenciales.nombreUsuario, credenciales.clave);

        if (!resultado.exito) {
            setError(resultado.error || 'Ocurrió un error inesperado.');
            setCargandoSubmit(false);
        }
    };

    if (cargandoAuth || usuario) {
        return null;
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
                            <input id="nombreUsuario" 
                            type="text" 
                            value={credenciales.nombreUsuario} 
                            onChange={e => setCredenciales({...credenciales, nombreUsuario: e.target.value})} 
                            required disabled={cargandoSubmit}
                            />
                        </div>
                        <div className="campoFormulario">
                            <label htmlFor="clave">Contraseña</label>
                            <input id="clave" type="password" 
                            value={credenciales.clave} 
                            onChange={e => setCredenciales({...credenciales, clave: e.target.value})} 
                            required disabled={cargandoSubmit}
                            />
                        </div>
                        {error && <p className="mensajeError">{error}</p>}

                        <Boton type="submit" className="anchoCompleto" disabled={cargandoSubmit}>
                            {cargandoSubmit ? 'Iniciando...' : 'Iniciar Sesión'}
                        </Boton>
                    </form>
                </div>
            </section>

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