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
    metadata: {
        [key: string]: any; // Para campos personalizados como _imagen_destacada_id
    };
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
