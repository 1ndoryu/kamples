BEGIN;

CREATE TABLE IF NOT EXISTS publicaciones (
    id                 SERIAL PRIMARY KEY,
    autor_id           INT NOT NULL REFERENCES usuarios_ext(id) ON DELETE CASCADE,
    tipo               VARCHAR(20) NOT NULL DEFAULT 'social'
                       CHECK (tipo IN ('social', 'sample')),
    contenido          TEXT DEFAULT '',
    imagenes           TEXT[] DEFAULT '{}',
    imagenes_metadata  JSONB DEFAULT '{}'::jsonb,
    samples_adjuntos   INT[] DEFAULT '{}',
    repost_id          INT REFERENCES publicaciones(id) ON DELETE SET NULL,
    moderacion_estado  VARCHAR(20) DEFAULT 'pendiente',
    moderacion_detalle JSONB DEFAULT '{}',
    moderacion_razon   VARCHAR(255) DEFAULT '',
    total_likes        INT DEFAULT 0,
    total_comentarios  INT DEFAULT 0,
    total_reposts      INT DEFAULT 0,
    eliminado_en       TIMESTAMPTZ NULL,
    created_at         TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_publicaciones_autor       ON publicaciones (autor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_publicaciones_autor_created_opt
    ON publicaciones (autor_id, created_at DESC)
    WHERE moderacion_estado IS NULL OR moderacion_estado = 'aprobado';
CREATE INDEX IF NOT EXISTS idx_publicaciones_created     ON publicaciones (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_publicaciones_moderacion  ON publicaciones (moderacion_estado);
CREATE INDEX IF NOT EXISTS idx_publicaciones_papelera
    ON publicaciones (eliminado_en) WHERE eliminado_en IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uk_publicaciones_repost_usuario
    ON publicaciones (autor_id, repost_id) WHERE repost_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS likes (
    usuario_id  INT NOT NULL REFERENCES usuarios_ext(id) ON DELETE CASCADE,
    tipo        VARCHAR(20) NOT NULL
                CHECK (tipo IN ('sample', 'publicacion', 'comentario', 'cancion', 'relacion')),
    target_id   INT NOT NULL,
    reaccion    VARCHAR(20) DEFAULT 'like'
                CHECK (reaccion IN ('like', 'dislike', 'encanta')),
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (usuario_id, tipo, target_id)
);

CREATE INDEX IF NOT EXISTS idx_likes_target          ON likes (tipo, target_id);
CREATE INDEX IF NOT EXISTS idx_likes_reaccion        ON likes (tipo, target_id, reaccion);
CREATE INDEX IF NOT EXISTS idx_likes_usuario_created ON likes (usuario_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_likes_usuario_tipo_target ON likes (usuario_id, tipo, target_id);
CREATE INDEX IF NOT EXISTS idx_likes_usuario_tipo_target_opt
    ON likes (usuario_id, tipo, target_id) INCLUDE (reaccion, created_at);
CREATE INDEX IF NOT EXISTS idx_likes_cancion   ON likes (target_id) WHERE tipo = 'cancion';
CREATE INDEX IF NOT EXISTS idx_likes_relacion  ON likes (target_id) WHERE tipo = 'relacion';
CREATE INDEX IF NOT EXISTS idx_likes_trending_24h
    ON likes (target_id, created_at DESC) WHERE tipo = 'sample';

CREATE TABLE IF NOT EXISTS follows (
    seguidor_id INT NOT NULL REFERENCES usuarios_ext(id) ON DELETE CASCADE,
    seguido_id  INT NOT NULL REFERENCES usuarios_ext(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (seguidor_id, seguido_id)
);

CREATE INDEX IF NOT EXISTS idx_follows_seguido  ON follows (seguido_id);
CREATE INDEX IF NOT EXISTS idx_follows_seguidor ON follows (seguidor_id, seguido_id);

/* [15A-2] Aqui vivia la tabla legacy `colecciones_guardadas` (coleccion_id INTEGER
 * REFERENCES colecciones(id)). Se elimina porque su FK apuntaba a una tabla que
 * no existe todavia en este punto: `colecciones` la crea 20260418000030, muy
 * posterior. La tabla canonica la crea 20260418000031 (DROP + CREATE, con
 * coleccion_id BIGINT), asi que en una base nueva esto no cambia el resultado
 * final y es lo que permite aplicar el juego de migraciones desde cero. */

CREATE TABLE IF NOT EXISTS comentarios (
    id                 SERIAL PRIMARY KEY,
    autor_id           INT NOT NULL REFERENCES usuarios_ext(id) ON DELETE CASCADE,
    tipo               VARCHAR(20) NOT NULL
                       CHECK (tipo IN ('sample', 'publicacion', 'cancion', 'relacion', 'articulo')),
    target_id          INT NOT NULL,
    contenido          TEXT,
    tipo_contenido     VARCHAR(20) DEFAULT 'texto',
    media_url          TEXT,
    media_metadata     JSONB,
    parent_id          INT REFERENCES comentarios(id) ON DELETE CASCADE,
    moderacion_estado  VARCHAR(20) DEFAULT 'aprobado',
    moderacion_detalle JSONB DEFAULT '{}',
    total_respuestas   INT DEFAULT 0,
    total_likes        INT DEFAULT 0,
    created_at         TIMESTAMPTZ DEFAULT NOW(),
    updated_at         TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_comentarios_target
    ON comentarios (tipo, target_id, created_at);
CREATE INDEX IF NOT EXISTS idx_comentarios_tipo_target_created_opt
    ON comentarios (tipo, target_id, created_at ASC)
    WHERE moderacion_estado IS NULL OR moderacion_estado != 'rechazado';
CREATE INDEX IF NOT EXISTS idx_comentarios_autor
    ON comentarios (autor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_comentarios_parent      ON comentarios (parent_id);
CREATE INDEX IF NOT EXISTS idx_comentarios_moderacion  ON comentarios (moderacion_estado);
CREATE INDEX IF NOT EXISTS idx_comentarios_cancion
    ON comentarios (target_id, created_at) WHERE tipo = 'cancion';
CREATE INDEX IF NOT EXISTS idx_comentarios_relacion
    ON comentarios (target_id, created_at) WHERE tipo = 'relacion';

COMMIT;


