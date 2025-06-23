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

export default function ImagenSample({nombre, src, alt, tamaño = 40, radio = 8}: Props) {
	const [fuenteAvatar, setFuenteAvatar] = useState<string | null>(null);

	useEffect(() => {
		if (!src) {
			// FIX: Se pasa un string vacío a `text` para generar el gradiente sin iniciales.
			getAvatar({name: nombre, text: '', size: tamaño, rounded: radio})
				.then(setFuenteAvatar)
				.catch(console.error);
		}
	}, [nombre, src, tamaño, radio]);

	const imagenFinal = src || fuenteAvatar;

	if (!imagenFinal) {
		return (
			<div
				className="imagenPlaceholder"
				style={{
					width: `${tamaño}px`,
					height: `${tamaño}px`,
					borderRadius: `${radio}px`,
					backgroundColor: 'var(--color-fondo-secundario)',
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
			style={{width: `${tamaño}px`, height: `${tamaño}px`, borderRadius: `${radio}px`, objectFit: 'cover'}}
		/>
	);
}