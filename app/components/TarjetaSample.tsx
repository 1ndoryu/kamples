// app/components/TarjetaSample.tsx
'use client';

import type {Sample} from '@/types/sample';
import Link from 'next/link';
import ImagenSample from '@/components/ui/ImagenSample';
import {ContenedorMenu, BotonMenu, Menu, MenuItem} from '@/components/ui/MenuDesplegable';
import ReproductorSample from '@/components/audio/ReproductorSample';

interface Props {
    sample: Sample;
}

// Función para procesar y construir la URL de la imagen
function obtenerUrlImagen(sample: Sample): string | undefined {
    const idImagen = sample.metadata._imagen_destacada_id;
    if (!idImagen) return undefined;
    // Asumimos una estructura de URL, esto puede necesitar ajuste
    // Por ahora, lo dejamos como placeholder ya que no tengo la estructura de URL de imágenes
    // return `${process.env.NEXT_PUBLIC_SWORD_BASE_URL}/path/to/image/${idImagen}.jpg`;
    // **Temporalmente, usamos una imagen de ejemplo si hay ID, hasta tener la URL real**
    return `https://picsum.photos/seed/${idImagen}/40/40`;
}

// Función para organizar los tags según las nuevas reglas
function obtenerTagsFormateados(sample: Sample): string[] {
    const {metadata} = sample;
    const tagsFormateados: string[] = [];

    // 1. Tipo
    if (metadata.tipo) tagsFormateados.push(metadata.tipo as string);

    // 2. Primer género
    if (metadata.genero) tagsFormateados.push(metadata.genero.split(',')[0].trim());

    // 3. Emoción
    if (metadata.emocion) tagsFormateados.push(metadata.emocion.split(',')[0].trim());

    // 4. Un tag adicional de la lista 'tags'
    if (metadata.tags) {
        const otrosTags = metadata.tags.split(',').map(t => t.trim());
        const tagAdicional = otrosTags.find(t => !tagsFormateados.includes(t));
        if (tagAdicional) tagsFormateados.push(tagAdicional);
    }

    return tagsFormateados.slice(0, 4); // Aseguramos un máximo de 4 tags
}

export default function TarjetaSample({sample}: Props) {
    const imageUrl = obtenerUrlImagen(sample);
    const tags = obtenerTagsFormateados(sample);

    return (
        <article className="tarjetaSample">
            {/* Contenedor del reproductor */}
            <div className="reproductorContenedor">
                <ReproductorSample sample={sample} />
            </div>

            {/* Info y Tags */}
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

            {/* Contenedor de la imagen */}
            <div className="imagenContenedor">
                <ImagenSample src={imageUrl} nombre={sample.titulo} alt={`Cover de ${sample.titulo}`} tamaño={40} radio={4} />
            </div>

            {/* Acciones */}
            <div className="accionesSample">
                <span className="infoBpm">{sample.metadata.bpm ? `${sample.metadata.bpm}bpm` : ''}</span>
                <ContenedorMenu>
                    <BotonMenu className="botonAcciones">•••</BotonMenu>
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
                    padding: 0.70rem;
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    transition: background-color 0.2s, border-color 0.2s;
                }
                .tarjetaSample:hover {
                    background-color: var(--color-tarjeta-fondo-hover);
                    border-color: var(--color-borde-hover, var(--color-borde));
                }

                .reproductorContenedor {
                    display: flex;
                    align-items: center;
                    flex-grow: 1; /* El reproductor ocupa el espacio disponible */
                    min-width: 0; /* Permite que el contenedor se encoja */
                }

                .infoPrincipal {
                    display: flex;
                    flex-direction: column;
                    gap: 0.25rem;
                    overflow: hidden;
                }

                .imagenContenedor {
                    margin-left: auto; /* Empuja la imagen hacia la derecha */
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
