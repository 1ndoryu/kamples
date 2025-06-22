// app/components/forms/FormularioSubirSample.tsx
'use client';

import {useState} from 'react';
import Input from '@/components/ui/Input';
import Boton from '@/components/ui/Boton';
import {useRouter} from 'next/navigation';

interface Props {
    alCerrar: () => void;
}

export default function FormularioSubirSample({alCerrar}: Props) {
    const [titulo, setTitulo] = useState('');
    const [archivo, setArchivo] = useState<File | null>(null);
    const [error, setError] = useState('');
    const [cargando, setCargando] = useState(false);
    const router = useRouter();

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
        formData.append('archivoSample', archivo);
        formData.append('tipocontenido', 'sample');
        formData.append('estado', 'publicado');

        try {
            // --- CORRECCIÓN CLAVE ---
            // La URL ahora incluye el segmento /auth/ para coincidir con la estructura de archivos.
            const respuesta = await fetch('/api/auth/samples/upload', {
                method: 'POST',
                body: formData
                // No se necesita 'Content-Type', el navegador lo pone automáticamente
            });

            const datos = await respuesta.json();

            if (!respuesta.ok) {
                // El mensaje de error viene de nuestra API proxy
                throw new Error(datos.error?.message || 'Error desconocido al subir el sample.');
            }

            // Si todo fue bien:
            console.log('Sample subido con éxito:', datos.data);
            alCerrar(); // Cierra el modal
            router.refresh(); // Refresca los datos de la página actual para ver el nuevo sample
        } catch (err: any) {
            setError(err.message);
        } finally {
            setCargando(false);
        }
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
                    margin-top: 0.5rem;
                }
            `}</style>
        </form>
    );
}