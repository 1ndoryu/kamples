/* ffmpeg y ffprobe que viajan con la app.
 *
 * [por que] Sin ffmpeg el pipeline de audio deja los samples en `procesando`
 * para siempre: no hay MP3 optimizado ni waveform, y el sample nunca aparece en
 * el catálogo. El backend los busca por `FFMPEG_PATH`/`FFPROBE_PATH`, así que
 * basta con apuntarlos al `bin/` del paquete antes de lanzarlo.
 */

use crate::escritorio::ajustes::{exigir, Rutas};
use std::path::PathBuf;

/// Rutas de los dos binarios, ya comprobadas.
pub struct Herramientas {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Herramientas {
    pub fn buscar(r: &Rutas) -> Result<Self, String> {
        let dir = r.ffmpeg_dir();
        let ffmpeg = dir.join("ffmpeg.exe");
        let ffprobe = dir.join("ffprobe.exe");
        exigir(&ffmpeg, "ffmpeg.exe")?;
        exigir(&ffprobe, "ffprobe.exe")?;
        Ok(Self { ffmpeg, ffprobe })
    }
}
