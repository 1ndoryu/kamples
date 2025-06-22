// app/components/forms/FormularioSubirSample.tsx
'use client';

import {useState} from 'react';
import {useRouter} from 'next/navigation';
import InfoUsuario from '@/components/ui/InfoUsuario';
import ZonaArrastre from './previews/ZonaArrastre';
import Boton from '../ui/Boton';

interface Props {
    alCerrar: () => void;
}

export default function FormularioSubirSample({alCerrar}: Props) {
    const [contenido, setContenido] = useState('');
    const [archivos, setArchivos] = useState<File[]>([]);
    const [error, setError] = useState('');
    const [cargando, setCargando] = useState(false);
    const router = useRouter();

    const manejarSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (archivos.length === 0) {
            setError('Por favor, añade al menos un archivo de audio.');
            return;
        }

        setError('');
        setCargando(true);

        const formData = new FormData();
        // Extraer tags del contenido
        const tags = contenido.match(/#(\w+)/g)?.map(tag => tag.substring(1)) || [];

        formData.append('contenido', contenido);
        formData.append('post_tags', JSON.stringify(tags)); // Enviamos los tags como un JSON array

        // Asumiendo que el primer audio es el principal y el resto son adjuntos
        // La API necesita esta lógica definida. Por ahora, adjuntamos todos.
        archivos.forEach((archivo, index) => {
            // El nombre del campo debe coincidir con la API. Usaré 'archivos[]' como convención.
            formData.append(`archivos[${index}]`, archivo);
        });

        // Aquí puedes agregar la lógica de los checkboxes
        // formData.append('fancheck', '1');

        try {
            // NOTA: El endpoint '/api/auth/samples/upload' debe ser actualizado
            // para manejar múltiples archivos y el nuevo formato de datos.
            const respuesta = await fetch('/api/auth/samples/upload', {
                method: 'POST',
                body: formData
            });

            const datos = await respuesta.json();

            if (!respuesta.ok) {
                throw new Error(datos.error?.message || 'Error al subir el post.');
            }

            console.log('Post creado con éxito:', datos.data);
            alCerrar();
            router.refresh();
        } catch (err: unknown) {
            setError(err instanceof Error ? err.message : 'Ocurrió un error inesperado.');
        } finally {
            setCargando(false);
        }
    };

    return (
        // Usamos la estructura HTML solicitada con clases para que apliques tus estilos
        <form className="bloqueFormulario" onSubmit={manejarSubmit} style={{display: 'flex', flexDirection: 'column', gap: '1rem'}}>
            <InfoUsuario />

            <div>
                {/* Este `textarea` simula el `contenteditable` y es más fácil de controlar en React */}
                <textarea id="textoRs" value={contenido} onChange={e => setContenido(e.target.value)} placeholder="Agrega tags usando #, puedes agregar varios audios a la vez" className="postTagsDABVYT" rows={1}></textarea>
            </div>

            <ZonaArrastre archivos={archivos} setArchivos={setArchivos} />

            {/* Placeholder para los checkboxes y opciones adicionales */}
            <div id="opcionesAdicionales" style={{display: 'none'}}>
                {/* Aquí irían los checkboxes de la estructura HTML */}
            </div>

            {error && <p className="mensajeError">{error}</p>}

            <div className="botonesForm">
                {/* Puedes añadir los botones de icono aquí si lo deseas */}
                <Boton type="button" variante="secundario" onClick={alCerrar} disabled={cargando}>
                    Cancelar
                </Boton>
                <Boton type="submit" variante="secundario" disabled={archivos.length === 0 || cargando}>
                    {cargando ? 'Publicando...' : 'Publicar'}
                </Boton>
            </div>

            <style jsx>{`
                .botonesForm {
                    display: flex;
                    justify-content: flex-end;
                    gap: 1rem;
                }
                .mensajeError {
                    color: #e53e3e;
                    font-size: 0.9rem;
                    text-align: center;
                }

                textarea#textoRs {
                    border-bottom: unset !important;
                }
            `}</style>
        </form>
    );
}
