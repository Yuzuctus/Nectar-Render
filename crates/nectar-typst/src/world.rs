//! Le monde Typst de Nectar : un système de fichiers virtuel, sans réseau.
//!
//! | Chemin virtuel     | Contenu                                   |
//! |--------------------|-------------------------------------------|
//! | `/main.typ`        | la source générée                         |
//! | `/nectar/…`        | aides, thème et fichiers du thème (embarqués) |
//! | `/assets/…`        | images de la note, lues sur le disque     |

use std::collections::HashMap;
use std::sync::Arc;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};
use typst_kit::fonts::FontStore;

pub(crate) fn file_id(path: &str) -> FileId {
    let vpath = VirtualPath::new(path).expect("chemin virtuel valide");
    FileId::new(RootedPath::new(VirtualRoot::Project, vpath))
}

pub(crate) struct NectarWorld {
    library: Arc<LazyHash<Library>>,
    fonts: Arc<FontStore>,
    main: FileId,
    /// Sources Typst en mémoire (principale, aides, thème).
    sources: HashMap<FileId, Source>,
    /// Fichiers binaires embarqués (thème de coloration…).
    embedded: HashMap<FileId, Bytes>,
    /// Images de la note : chemin virtuel → fichier réel ou contenu en mémoire.
    assets: HashMap<FileId, nectar_core::Asset>,
    /// Images déjà lues, partagées entre compilations.
    cache: Arc<crate::images::ImageCache>,
}

impl NectarWorld {
    pub(crate) fn new(
        library: Arc<LazyHash<Library>>,
        fonts: Arc<FontStore>,
        cache: Arc<crate::images::ImageCache>,
        main_source: String,
        texts: &[(&str, &str)],
        binaries: &[(&str, &'static [u8])],
        assets: &[nectar_core::Asset],
    ) -> Self {
        let main = file_id("/main.typ");
        let mut sources = HashMap::new();
        sources.insert(main, Source::new(main, main_source));
        for (path, text) in texts {
            let id = file_id(path);
            sources.insert(id, Source::new(id, (*text).to_string()));
        }
        let embedded = binaries.iter().map(|(path, data)| (file_id(path), Bytes::new(*data))).collect();
        let assets = assets.iter().map(|a| (file_id(&a.vpath), a.clone())).collect();
        Self { library, fonts, main, sources, embedded, assets, cache }
    }

    pub(crate) fn main_source(&self) -> &Source {
        &self.sources[&self.main]
    }

    fn read_asset(&self, id: FileId) -> FileResult<Bytes> {
        let asset = self.assets.get(&id).ok_or_else(|| not_found(id))?;
        if let Some(data) = &asset.data {
            return Ok(Bytes::from_string(data.as_str().to_owned()));
        }
        self.cache.get(&asset.path, asset.max_px).map_err(|e| FileError::from_io(e, &asset.path))
    }
}

fn not_found(id: FileId) -> FileError {
    FileError::NotFound(id.vpath().get_with_slash().into())
}

impl World for NectarWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.sources.get(&id).cloned().ok_or_else(|| not_found(id))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(source) = self.sources.get(&id) {
            return Ok(Bytes::from_string(source.clone()));
        }
        if let Some(bytes) = self.embedded.get(&id) {
            return Ok(bytes.clone());
        }
        self.read_asset(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
        let offset = offset.map(|d| d.seconds() as i64).unwrap_or(0);
        let (y, m, d) = civil_from_days((secs + offset).div_euclid(86_400));
        Datetime::from_ymd(y, m, d)
    }
}

/// Date et heure UTC actuelles.
pub(crate) fn now_utc() -> Option<Datetime> {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    Datetime::from_ymd_hms(y, m, d, (rest / 3600) as u8, (rest % 3600 / 60) as u8, (rest % 60) as u8)
}

/// Jours depuis 1970-01-01 → date civile (algorithme de H. Hinnant).
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as i32, m as u8, d as u8)
}

#[cfg(test)]
mod tests {
    #[test]
    fn civil_dates() {
        assert_eq!(super::civil_from_days(0), (1970, 1, 1));
        assert_eq!(super::civil_from_days(20_362), (2025, 10, 1));
    }
}
