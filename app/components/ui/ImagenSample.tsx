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

// Función para generar iniciales a partir de un nombre
const obtenerIniciales = (nombre: string): string => {
	if (!nombre) return '';
	const palabras = nombre.split(' ');
	if (palabras.length > 1) {
		return `${palabras[0][0]}${palabras[1][0]}`.toUpperCase();
	}
	return nombre.substring(0, 2).toUpperCase();
};

export default function ImagenSample({nombre, src, alt, tamaño = 80, radio = 8}: Props) {
	const [fuenteAvatar, setFuenteAvatar] = useState<string>('');

	useEffect(() => {
		if (!src) {
			// CORRECCIÓN: Añadimos la propiedad 'text' obligatoria.
			const iniciales = obtenerIniciales(nombre);
			getAvatar({name: nombre, text: iniciales, size: tamaño, rounded: radio})
				.then(setFuenteAvatar)
				.catch(console.error);
		}
	}, [nombre, src, tamaño, radio]);

	// Se mantiene el uso de Next/Image para optimización
	// El tamaño se pasa directamente para evitar 'layout shift'
	return (
		<Image
			src={src || fuenteAvatar}
			alt={alt}
			width={tamaño}
			height={tamaño}
			className="imagenSample"
			style={{width: `${tamaño}px`, height: `${tamaño}px`}}
		/>
	);
}