// app/components/forms/FormularioSubirSample.tsx
'use client';

import {useState, useRef, useCallback} from 'react';
import {useRouter} from 'next/navigation';
import InfoUsuario from '@/components/ui/InfoUsuario';
import ZonaArrastre from './previews/ZonaArrastre';
import Boton from '../ui/Boton';

interface Props {
    alCerrar: () => void;
}

export default function FormularioSubirSample({alCerrar}: Props) {
    const [contenido, setContenido] = useState('');
    const [archivoAudio, setArchivoAudio] = useState<File | null>(null);
    const [archivoImagen, setArchivoImagen] = useState<File | null>(null);

    const [error, setError] = useState('');
    const [cargando, setCargando] = useState(false);
    const [estaArrastrando, setEstaArrastrando] = useState(false);
    const router = useRouter();

    const inputAudioRef = useRef<HTMLInputElement>(null);
    const inputImagenRef = useRef<HTMLInputElement>(null);

    const manejarSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!archivoAudio) {
            setError('El archivo de audio es obligatorio.');
            return;
        }

        setError('');
        setCargando(true);

        const formData = new FormData();
        const tags = contenido.match(/#(\w+)/g)?.map(tag => tag.substring(1)) || [];

        formData.append('archivoSample', archivoAudio);
        if (archivoImagen) {
            formData.append('archivoImagen', archivoImagen);
        }

        formData.append('titulo', contenido.trim() || archivoAudio.name.replace(/\.[^/.]+$/, ''));
        formData.append('tipocontenido', 'sample');
        formData.append('estado', 'publicado');
        formData.append('post_tags', JSON.stringify(tags));

        try {
            const respuesta = await fetch('/api/auth/samples/upload', {
                method: 'POST',
                body: formData
            });

            const datos = await respuesta.json();

            if (!respuesta.ok) {
                throw new Error(datos.error?.message || 'Error al subir el sample.');
            }

            alCerrar();
            router.refresh();
        } catch (err: unknown) {
            setError(err instanceof Error ? err.message : 'Ocurrió un error inesperado.');
        } finally {
            setCargando(false);
        }
    };

    const manejarArrastre = (e: React.DragEvent<HTMLFormElement>, activo: boolean) => {
        e.preventDefault();
        e.stopPropagation();
        setEstaArrastrando(activo);
    };

    const manejarSoltar = useCallback((e: React.DragEvent<HTMLFormElement>) => {
        e.preventDefault();
        e.stopPropagation();
        setEstaArrastrando(false);
        setError('');

        if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
            const archivosSoltados = Array.from(e.dataTransfer.files);
            const audio = archivosSoltados.find(f => f.type.startsWith('audio/'));
            const imagen = archivosSoltados.find(f => f.type.startsWith('image/'));

            if (audio) setArchivoAudio(audio);
            if (imagen) setArchivoImagen(imagen);

            if (!audio && !imagen) {
                setError('Solo puedes arrastrar archivos de audio o imagen.');
            }
        }
    }, []);

    const manejarSeleccionArchivo = (e: React.ChangeEvent<HTMLInputElement>, tipo: 'audio' | 'imagen') => {
        if (e.target.files && e.target.files.length > 0) {
            const archivo = e.target.files[0];
            if (tipo === 'audio') {
                setArchivoAudio(archivo);
            } else {
                setArchivoImagen(archivo);
            }
        }
    };

    // Variable para controlar la visibilidad del preview
    const hayArchivos = archivoAudio || archivoImagen;

    return (
        <form className={`bloqueFormulario ${estaArrastrando ? 'arrastreActivo' : ''}`} onSubmit={manejarSubmit} onDragEnter={e => manejarArrastre(e, true)} onDragOver={e => manejarArrastre(e, true)} onDragLeave={e => manejarArrastre(e, false)} onDrop={manejarSoltar}>
            <InfoUsuario />

            <textarea id="textoRs" value={contenido} onChange={e => setContenido(e.target.value)} placeholder="Describe tu sample, usa #tags para clasificar..." className="postTagsDABVYT" rows={1}></textarea>

            {/* AJUSTE: La zona de preview solo se muestra si hay archivos */}
            {hayArchivos && <ZonaArrastre archivoAudio={archivoAudio} archivoImagen={archivoImagen} onAdjuntarAudio={() => inputAudioRef.current?.click()} onAdjuntarImagen={() => inputImagenRef.current?.click()} onEliminarAudio={() => setArchivoAudio(null)} onEliminarImagen={() => setArchivoImagen(null)} />}

            <input type="file" ref={inputAudioRef} onChange={e => manejarSeleccionArchivo(e, 'audio')} accept="audio/*" style={{display: 'none'}} />
            <input type="file" ref={inputImagenRef} onChange={e => manejarSeleccionArchivo(e, 'imagen')} accept="image/*" style={{display: 'none'}} />

            {error && <p className="mensajeError">{error}</p>}

            {/* AJUSTE: Contenedor para TODOS los botones de acción */}
            <div className="accionesFormulario">
                {/* Botones para adjuntar archivos */}
                <div className="botonesAdjuntar">
                    <Boton type="button" onClick={() => inputAudioRef.current?.click()} variante="secundario">
                        {archivoAudio ? 'Cambiar Audio' : 'Adjuntar Audio'}
                    </Boton>
                    <Boton type="button" onClick={() => inputImagenRef.current?.click()} variante="secundario">
                        {archivoImagen ? 'Cambiar Imagen' : 'Adjuntar Imagen'}
                    </Boton>
                </div>

                {/* Botones para enviar o cancelar */}
                <div className="botonesPublicar">
                    <Boton type="button" variante="secundario" onClick={alCerrar} disabled={cargando}>
                        Cancelar
                    </Boton>
                    <Boton type="submit" variante="secundario" disabled={!archivoAudio || cargando}>
                        {cargando ? 'Publicando...' : 'Publicar'}
                    </Boton>
                </div>
            </div>

            <style jsx>{`
                .bloqueFormulario {
                    display: flex;
                    flex-direction: column;
                    gap: 0.5rem;
                    transition: background-color 0.2s;
                    border-radius: var(--radius);
                    max-width: 600px;
                    width: 100%;
                }
                .bloqueFormulario.arrastreActivo {
                    background-color: rgba(var(--color-primario-rgb), 0.1);
                    border-color: var(--color-primario);
                }
                .accionesFormulario {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-top: 0.5rem;
                }
                .botonesAdjuntar,
                .botonesPublicar {
                    display: flex;
                    gap: 0.5rem;
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
