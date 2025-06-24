// app/components/forms/FormularioSubirSample.tsx
'use client';

import {useState, useRef, useCallback} from 'react';
import {useRouter} from 'next/navigation';
import InfoUsuario from '@/components/ui/InfoUsuario';
import ZonaArrastre from './previews/ZonaArrastre';
import Boton from '../ui/Boton';
import { useAppStore } from '@/store/useAppStore'; // Importar el store

interface Props {
    alCerrar: () => void;
}

export default function FormularioSubirSample({alCerrar}: Props) {
    const [contenido, setContenido] = useState(''); // Mantener para el textarea
    const [archivoAudio, setArchivoAudio] = useState<File | null>(null); // Mantener para los archivos
    const [archivoImagen, setArchivoImagen] = useState<File | null>(null); // Mantener para los archivos

    // const [error, setError] = useState(''); // Reemplazado por errorSamples del store
    // const [cargando, setCargando] = useState(false); // Reemplazado por isLoadingSamples del store
    const isLoadingSamples = useAppStore((state) => state.isLoadingSamples); // Estado de carga del store
    const errorSamples = useAppStore((state) => state.errorSamples); // Errores del store
    const uploadSampleStore = useAppStore((state) => state.uploadSampleStore);
    const setErrorSamples = useAppStore((state) => state.setSamplesError); // Para limpiar errores manualmente si es necesario

    const [estaArrastrando, setEstaArrastrando] = useState(false); // UI local
    const router = useRouter();

    const inputAudioRef = useRef<HTMLInputElement>(null);
    const inputImagenRef = useRef<HTMLInputElement>(null);

    const manejarSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!archivoAudio) {
            // Usar el setter de error del store o un estado local si se prefiere para errores de validación de form
            setErrorSamples('El archivo de audio es obligatorio.');
            return;
        }

        setErrorSamples(null); // Limpiar errores previos del store

        const formData = new FormData();
        const tags = contenido.match(/#(\w+)/g)?.map(tag => tag.substring(1)) || [];

        formData.append('archivoSample', archivoAudio);
        if (archivoImagen) {
            formData.append('archivoImagen', archivoImagen);
        }
        // El título se extrae del 'contenido' o del nombre del archivo si 'contenido' está vacío.
        // El campo 'contenido' aquí es el que se usa para el título y la descripción/tags.
        const tituloSample = contenido.trim().split('\n')[0] || archivoAudio.name.replace(/\.[^/.]+$/, '');
        formData.append('titulo', tituloSample);
        formData.append('tipocontenido', 'sample'); // Podría ser configurable
        formData.append('estado', 'publicado'); // Podría ser configurable
        formData.append('post_tags', JSON.stringify(tags)); // Los tags se envían como un string JSON
        // formData.append('descripcion', contenido.trim()); // Si 'contenido' es más que solo tags/título

        const resultado = await uploadSampleStore(formData);

        if (resultado.success) {
            alCerrar(); // Cierra el modal/formulario
            router.refresh(); // Refresca la página para ver el nuevo sample (si es necesario, el store ya se actualizó)
            // Opcionalmente, podrías redirigir o mostrar un mensaje de éxito.
        } else {
            // El error ya está en errorSamples del store, se mostrará automáticamente.
            // Si setErrorSamples no fue usado para el error de validación, no es necesario limpiarlo aquí.
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
        setErrorSamples(null); // Limpiar error al intentar una nueva acción

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

            {errorSamples && <p className="mensajeError">{errorSamples}</p>} {/* Mostrar error del store */}

            {/* AJUSTE: Contenedor para TODOS los botones de acción */}
            <div className="accionesFormulario">
                {/* Botones para adjuntar archivos */}
                <div className="botonesAdjuntar">
                    <Boton type="button" onClick={() => inputAudioRef.current?.click()} variante="secundario" disabled={isLoadingSamples}>
                        {archivoAudio ? 'Cambiar Audio' : 'Adjuntar Audio'}
                    </Boton>
                    <Boton type="button" onClick={() => inputImagenRef.current?.click()} variante="secundario" disabled={isLoadingSamples}>
                        {archivoImagen ? 'Cambiar Imagen' : 'Adjuntar Imagen'}
                    </Boton>
                </div>

                {/* Botones para enviar o cancelar */}
                <div className="botonesPublicar">
                    <Boton type="button" variante="secundario" onClick={alCerrar} disabled={isLoadingSamples}>
                        Cancelar
                    </Boton>
                    <Boton type="submit" variante="secundario" disabled={!archivoAudio || isLoadingSamples}>
                        {isLoadingSamples ? 'Publicando...' : 'Publicar'}
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
