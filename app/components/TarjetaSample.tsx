// app/components/TarjetaSample.tsx
'use client';

import type {Sample} from '@/types/sample';
import Link from 'next/link';
import ImagenSample from '@/components/ui/ImagenSample';
import {ContenedorMenu, BotonMenu, Menu, MenuItem} from '@/components/ui/MenuDesplegable';

interface Props {
	sample: Sample;
}

export default function TarjetaSample({sample}: Props) {
	// Por ahora, no hay una URL de imagen, así que `ImagenSample` generará el gradiente.
	const imageUrl = undefined;
	const tags = sample.metadata.tags?.split(',').map(t => t.trim()) || [];

	return (
		<article className="tarjetaSample">
			<div className="imagenContenedor">
				<ImagenSample src={imageUrl} nombre={sample.titulo} alt={`Cover de ${sample.titulo}`} tamaño={80} radio={4} />
				<button className="botonPlay" aria-label={`Reproducir ${sample.titulo}`}>
					▶
				</button>
			</div>

			<div className="infoPrincipal">
				<Link href={`/samples/${sample.slug}`} className="enlaceTitulo">
					<h3 className="tituloSample">{sample.titulo}</h3>
				</Link>
				<div className="tagsContenedor">
					{tags.slice(0, 3).map(tag => (
						<span key={tag} className="tag">
							{tag}
						</span>
					))}
				</div>
			</div>

			<div className="accionesSample">
				<span className="infoBpm">{sample.metadata.bpm ? `${sample.metadata.bpm}bpm` : ''}</span>
				<ContenedorMenu>
					<BotonMenu className="botonAcciones">•••</BotonMenu>
					<Menu ancho={180}>
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
					border: 1px solid var(--color-borde);
					border-radius: var(--radius);
					padding: 0.75rem;
					display: flex;
					align-items: center;
					gap: 1rem;
					transition: background-color 0.2s, border-color 0.2s;
				}
				.tarjetaSample:hover {
					background-color: var(--color-tarjeta-fondo-hover);
					border-color: var(--color-borde-hover, var(--color-borde));
				}
				.imagenContenedor {
					position: relative;
					cursor: pointer;
				}
				.botonPlay {
					position: absolute;
					top: 50%;
					left: 50%;
					transform: translate(-50%, -50%);
					background-color: rgba(0, 0, 0, 0.5);
					color: white;
					border: none;
					border-radius: 50%;
					width: 40px;
					height: 40px;
					display: flex;
					align-items: center;
					justify-content: center;
					font-size: 1.2rem;
					opacity: 0;
					transition: opacity 0.2s;
				}
				.imagenContenedor:hover .botonPlay {
					opacity: 1;
				}
				.infoPrincipal {
					flex-grow: 1;
					display: flex;
					flex-direction: column;
					gap: 0.25rem;
					overflow: hidden;
				}
				.enlaceTitulo {
					text-decoration: none;
					color: inherit;
				}
				.tituloSample {
					margin: 0;
					font-size: 1rem;
					white-space: nowrap;
					overflow: hidden;
					text-overflow: ellipsis;
				}
				.tagsContenedor {
					display: flex;
					gap: 0.4rem;
					flex-wrap: wrap;
				}
				.tag {
					font-size: 0.7rem;
					background-color: var(--color-fondo-secundario);
					padding: 0.15rem 0.4rem;
					border-radius: 4px;
					border: 1px solid var(--color-borde);
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