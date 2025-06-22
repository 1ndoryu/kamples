// app/components/ui/ImagenSample.tsx
'use client';

import {useEffect, useState} from 'react';
import {getAvatar} from '@simplr-sh/avatar';
import Image from 'next/image';

interface Props {
	nombre: string;
	src?: string | null;
	alt: string;
	tamaño?: number;
	radio?: number;
}

const obtenerIniciales = (nombre: string): string => {
	if (!nombre) return '';
	const palabras = nombre.split(' ');
	if (palabras.length > 1) {
		return `${palabras[0][0]}${palabras[1][0]}`.toUpperCase();
	}
	return nombre.substring(0, 2).toUpperCase();
};

export default function ImagenSample({nombre, src, alt, tamaño = 80, radio = 8}: Props) {
	// FIX: Se inicializa el estado con `null` para evitar pasar un string vacío.
	const [fuenteAvatar, setFuenteAvatar] = useState<string | null>(null);

	useEffect(() => {
		// Solo se genera un avatar si no se proporciona una prop `src`.
		if (!src) {
			const iniciales = obtenerIniciales(nombre);
			getAvatar({name: nombre, text: iniciales, size: tamaño, rounded: radio})
				.then(setFuenteAvatar)
				.catch(console.error);
		}
	}, [nombre, src, tamaño, radio]);

	const imagenFinal = src || fuenteAvatar;

	// FIX: Se renderiza un placeholder si no hay fuente de imagen final.
	// Esto evita el error de "src" vacío y previene un cambio de layout (CLS).
	if (!imagenFinal) {
		return (
			<div
				className="imagenPlaceholder"
				style={{
					width: `${tamaño}px`,
					height: `${tamaño}px`,
					borderRadius: `${radio}px`,
					backgroundColor: 'var(--color-fondo-secundario)', // Color de fondo para el placeholder
				}}
			/>
		);
	}

	return (
		<Image
			src={imagenFinal}
			alt={alt}
			width={tamaño}
			height={tamaño}
			className="imagenSample"
			// Se añade `objectFit` y se pasa el radio para asegurar consistencia visual.
			style={{width: `${tamaño}px`, height: `${tamaño}px`, borderRadius: `${radio}px`, objectFit: 'cover'}}
		/>
	);
}