// app/components/TarjetaSample.tsx
'use client';

import type {Sample} from '@/types/sample';
import Link from 'next/link';
import ImagenSample from '@/components/ui/ImagenSample';
import {ContenedorMenu, BotonMenu, Menu, MenuItem} from '@/components/ui/MenuDesplegable';
import Waveform from '@/components/audio/Waveform';

interface Props {
    sample: Sample;
}

function obtenerUrlImagen(sample: Sample): string | undefined {
    const idImagen = sample.metadata._imagen_destacada_id;
    const baseUrl = process.env.NEXT_PUBLIC_SWORD_BASE_URL;

    if (!idImagen || !baseUrl) return undefined;

    return `https://picsum.photos/seed/${idImagen}/40/40`;
}

function obtenerTagsFormateados(sample: Sample): string[] {
    const {metadata} = sample;
    const tagsFormateados: string[] = [];

    if (metadata.tipo) tagsFormateados.push(metadata.tipo as string);
    if (metadata.genero) tagsFormateados.push(metadata.genero.split(',')[0].trim());
    if (metadata.emocion) tagsFormateados.push(metadata.emocion.split(',')[0].trim());
    if (metadata.tags) {
        const otrosTags = metadata.tags.split(',').map(t => t.trim());
        const tagAdicional = otrosTags.find(t => !tagsFormateados.includes(t));
        if (tagAdicional) tagsFormateados.push(tagAdicional);
    }
    return tagsFormateados.slice(0, 5);
}

export default function TarjetaSample({sample}: Props) {
    const imageUrl = obtenerUrlImagen(sample);
    const tags = obtenerTagsFormateados(sample);
    const urlAudio = sample.metadata.url_archivo;

    return (
        <article className="tarjetaSample">
            <div className="imagenContenedor">
                <ImagenSample src={imageUrl} nombre={sample.titulo} alt={`Cover de ${sample.titulo}`} tamaño={40} radio={4} />
            </div>

            <div className="infoPrincipal">
                <Link href={`/samples/${sample.slug}`} className="enlaceTitulo">
                    <h3 className="tituloSample">{sample.titulo}</h3>
                </Link>
                <div className="tagsContenedor">
                    {tags.map(tag => (
                        <span key={tag} className="tag">
                            {tag}
                        </span>
                    ))}
                </div>
            </div>
            <span className="infoBpm">{sample.metadata.bpm ? `${sample.metadata.bpm}bpm` : ''}</span>

            <div className="reproductorContenedor">{urlAudio ? <Waveform idSample={sample.id} urlAudio={urlAudio} /> : <div className="reproductorPlaceholder">Audio no disponible</div>}</div>

            <div className="accionesSample">
                <ContenedorMenu>
                    <BotonMenu className="botonAcciones">• • •</BotonMenu>
                    <Menu ancho={150}>
                        <MenuItem onClick={() => alert('Descargando...')}>Descargar</MenuItem>
                        <MenuItem onClick={() => alert('Añadiendo a colección...')}>Añadir a Colección</MenuItem>
                        <MenuItem onClick={() => alert('Compartiendo...')}>Compartir</MenuItem>
                        <MenuItem onClick={() => alert('Reportando...')} tipo="peligro">
                            Reportar
                        </MenuItem>
                    </Menu>
                </ContenedorMenu>
            </div>

            <style jsx>{`
                .tarjetaSample {
                    border: var(--borde);
                    border-radius: var(--radius);
                    background: var(--fondo);
                    align-items: center;
                    gap: 1rem;
                    padding: 0.63rem;
                    transition: background-color 0.2s, border-color 0.2s;
                    display: flex;
                }
                .tarjetaSample:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                    border-color: var(--color-borde-hover, var(--color-borde));
                }
                .reproductorContenedor {
                    display: flex;
                    align-items: center;
                    min-width: 250px;
                }
                .reproductorPlaceholder {
                    font-size: 0.75rem;
                    opacity: 0.5;
                    flex-grow: 1;
                    padding-left: 1rem;
                }
                .infoPrincipal {
                    display: flex;
                    flex-direction: column;
                    gap: 0.25rem;
                    overflow: hidden;
                    margin-right: auto;
                }
                .imagenContenedor {
                    flex-shrink: 0;
                }
                .enlaceTitulo {
                    text-decoration: none;
                    color: inherit;
                }
                .tituloSample {
                    white-space: nowrap;
                    text-overflow: ellipsis;
                    margin: 0;
                    font-size: 0.8rem;
                    overflow: hidden;
                    font-weight: 500;
                    line-height: 11px;
                }
                .tagsContenedor {
                    display: flex;
                    gap: 0.6rem;
                    flex-wrap: wrap;
                }
                .tag {
                    font-size: 0.75rem;
                    opacity: 0.7;
                    cursor: pointer;
                }
                .accionesSample {
                    display: flex;
                    align-items: center;
                    gap: 0.75rem;
                    padding: 0rem 1rem;
                }
                .infoBpm {
                    font-size: 0.8rem;
                    font-weight: 500;
                    color: var(--color-texto);
                    opacity: 0.8;
                }
                .botonAcciones {
                    background: transparent;
                    border: none;
                    color: var(--color-texto);
                    font-size: 1.2rem;
                    cursor: pointer;
                    padding: 0.2rem 0.5rem;
                    border-radius: var(--radius);
                }
                .botonAcciones:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                }
            `}</style>
        </article>
    );
}
