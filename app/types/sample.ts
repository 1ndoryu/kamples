// app/types/sample.ts

// Interfaz específica para los metadatos que esperamos en un sample
export interface MetadataSample {
	bpm?: string;
	tags?: string; // Ej: "tag1,tag2,tag3"
	genero?: string;
	emocion?: string;
	media_id?: string | number;
	url_archivo?: string;
	instrumentos?: string;
	artista_vibes?: string;
	sugerencia_busqueda?: string;
	_imagen_destacada_id?: number;
	// Permite otras propiedades no definidas explícitamente
	[key: string]: unknown;
}

// Basado en la documentación de la API /content
export interface Sample {
	id: number;
	titulo: string;
	subtitulo: string | null;
	contenido: string;
	slug: string;
	idautor: number;
	estado: 'publicado' | 'borrador' | 'privado';
	tipocontenido: string; // 'sample', 'post', etc.
	metadata: MetadataSample; // Usamos la nueva interfaz
	created_at: string; // Formato ISO 8601
	updated_at: string; // Formato ISO 8601
}

export interface Paginacion {
	total_items: number;
	total_pages: number;
	current_page: number;
	per_page: number;
}

export interface RespuestaApiSamples {
	data: {
		items: Sample[];
		pagination: Paginacion;
	};
}