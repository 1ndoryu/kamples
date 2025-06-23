// app/types/sample.ts

// Interfaz específica para los metadatos que esperamos en un sample
export interface MetadataSample {
	// --- Campos de tipo string ---
	bpm?: string;
	tipo?: string;
	media_id?: string | number;
	url_archivo?: string;
	descripcion?: string;
	descripcion_corta?: string;
	_imagen_destacada_id?: number;
	url_imagen_destacada?: string; // Campo recomendado para la URL de la imagen

	// --- Campos de tipo array de strings ---
	tags?: string[];
	genero?: string[];
	emocion?: string[];
	instrumentos?: string[];
	artista_vibes?: string[];

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
	metadata: MetadataSample; // Usamos la interfaz actualizada
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