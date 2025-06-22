// app/components/forms/FormularioSubirSample.tsx
'use client';

import {useState} from 'react';
import Input from '@/components/ui/Input';
import Boton from '@/components/ui/Boton';

interface Props {
    alCerrar: () => void;
}

export default function FormularioSubirSample({alCerrar}: Props) {
    const [titulo, setTitulo] = useState('');
    const [archivo, setArchivo] = useState<File | null>(null);
    const [error, setError] = useState('');
    const [cargando, setCargando] = useState(false);

    const manejarSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!archivo) {
            setError('Por favor, selecciona un archivo de audio.');
            return;
        }

        setError('');
        setCargando(true);

        const formData = new FormData();
        formData.append('titulo', titulo);
        formData.append('archivoSample', archivo); // El archivo en sí
        formData.append('tipocontenido', 'sample');
        formData.append('estado', 'publicado'); // o 'borrador'

        /*
         * TODO: BACKEND
         * La API de SwordPHP necesita un endpoint que acepte `multipart/form-data`.
         * El endpoint actual `POST /content` solo acepta `application/json`.
         * Se debe crear un nuevo endpoint, por ejemplo `POST /content/upload` o modificar el existente.
         *
         * Cuando esté listo, aquí se haría la llamada fetch:
         *
         * try {
         * const respuesta = await fetch('/api/proxy-a-sword', {
         * method: 'POST',
         * body: formData,
         * // No se necesita 'Content-Type', el navegador lo pone automáticamente con FormData
         * });
         * const datos = await respuesta.json();
         * if (!respuesta.ok) throw new Error(datos.error.message);
         * * // Si todo fue bien:
         * console.log('Sample subido!', datos);
         * alCerrar(); // Cierra el modal
         *
         * } catch (err) {
         * setError(err.message);
         * } finally {
         * setCargando(false);
         * }
         */

        // Simulación temporal
        console.log('Enviando datos:', {
            titulo,
            archivo
        });
        setTimeout(() => {
            console.log('¡Simulación de subida completada!');
            setCargando(false);
            alCerrar();
        }, 1500);
    };

    return (
        <form onSubmit={manejarSubmit}>
            <Input id="titulo" label="Título del Sample" type="text" value={titulo} onChange={e => setTitulo(e.target.value)} required disabled={cargando} />

            <Input id="archivo" label="Archivo de Audio (.wav, .mp3, etc.)" type="file" accept="audio/*" onChange={e => setArchivo(e.target.files ? e.target.files[0] : null)} required disabled={cargando} />

            {error && <p className="mensajeError">{error}</p>}

            <div className="accionesFormulario">
                <Boton type="button" variante="secundario" onClick={alCerrar} disabled={cargando}>
                    Cancelar
                </Boton>
                <Boton type="submit" variante="primario" disabled={!titulo || !archivo || cargando}>
                    {cargando ? 'Subiendo...' : 'Crear Sample'}
                </Boton>
            </div>

            <style jsx>{`
                .mensajeError {
                    color: #e53e3e;
                    background-color: rgba(229, 62, 62, 0.1);
                    border-radius: 8px;
                    padding: 0.75rem;
                    text-align: center;
                    margin-bottom: 1.5rem;
                    font-size: 0.9rem;
                }
                .accionesFormulario {
                    display: flex;
                    justify-content: flex-end;
                    gap: 1rem;
                    margin-top: 0.5rem; /* Ajuste para reducir espacio extra del último input */
                }
            `}</style>
        </form>
    );
}
