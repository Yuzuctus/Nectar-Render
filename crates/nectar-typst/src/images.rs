//! Photos trop lourdes : réduites avant d'entrer dans le PDF.
//!
//! Une photo de téléphone (4000 × 3000 px) imprimée sur une demi-page n'a
//! besoin que d'environ 1800 px. Au-delà de la limite, on la redresse (sens
//! de prise de vue), on la réduit (Lanczos, accéléré par le processeur) et
//! on la réencode : JPEG qualité 88, PNG sans perte.
//!
//! Le résultat est gardé en mémoire et sur le disque : une photo n'est
//! réduite qu'une fois, même d'une ouverture de l'atelier à l'autre ; et les
//! photos d'une note se préparent en parallèle, une par cœur.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;

use fast_image_resize::images::Image as FastImage;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
use typst::foundations::Bytes;

/// Taille au-delà de laquelle le cache sur disque est élagué.
const DISK_BUDGET: u64 = 400 * 1024 * 1024;

/// Réduit l'image si son plus grand côté dépasse `max` pixels ; sinon, ou si
/// le format n'est pas géré, rend les octets tels quels.
pub fn downscale(bytes: Vec<u8>, max: u32) -> Vec<u8> {
    let Ok(reader) = ImageReader::new(Cursor::new(&bytes)).with_guessed_format() else { return bytes };
    let Some(format) = reader.format().filter(|f| matches!(f, ImageFormat::Png | ImageFormat::Jpeg)) else {
        return bytes;
    };
    let Ok((w, h)) = reader.into_dimensions() else { return bytes };
    if w.max(h) <= max {
        return bytes;
    }
    let Some(img) = decode_upright(&bytes, format) else { return bytes };
    let (w, h) = (img.width(), img.height());
    let ratio = f64::from(max) / f64::from(w.max(h));
    let (tw, th) = (((f64::from(w) * ratio).round() as u32).max(1), ((f64::from(h) * ratio).round() as u32).max(1));
    let out = match format {
        ImageFormat::Jpeg => {
            let rgb = img.to_rgb8();
            let Some(pixels) = resize(rgb.as_raw(), (w, h), (tw, th), PixelType::U8x3) else { return bytes };
            let mut out = Vec::new();
            let encoder = jpeg_encoder::Encoder::new(&mut out, 88);
            match encoder.encode(&pixels, tw as u16, th as u16, jpeg_encoder::ColorType::Rgb) {
                Ok(()) => out,
                Err(_) => return bytes,
            }
        }
        _ => {
            let rgba = img.to_rgba8();
            let Some(pixels) = resize(rgba.as_raw(), (w, h), (tw, th), PixelType::U8x4) else { return bytes };
            let Some(resized) = image::RgbaImage::from_raw(tw, th, pixels) else { return bytes };
            let mut out = Vec::new();
            if DynamicImage::ImageRgba8(resized).write_to(&mut Cursor::new(&mut out), ImageFormat::Png).is_err() {
                return bytes;
            }
            out
        }
    };
    if out.len() < bytes.len() { out } else { bytes }
}

/// Décode l'image et la redresse selon son orientation (EXIF), que le
/// réencodage ferait sinon perdre.
fn decode_upright(bytes: &[u8], format: ImageFormat) -> Option<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format);
    let mut decoder = reader.into_decoder().ok()?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).ok()?;
    if let Some(orientation) = orientation {
        img.apply_orientation(orientation);
    }
    Some(img)
}

fn resize(pixels: &[u8], from: (u32, u32), to: (u32, u32), kind: PixelType) -> Option<Vec<u8>> {
    let src = FastImage::from_vec_u8(from.0, from.1, pixels.to_vec(), kind).ok()?;
    let mut dst = FastImage::new(to.0, to.1, kind);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
    Resizer::new().resize(&src, &mut dst, &options).ok()?;
    Some(dst.into_vec())
}

/// Une image et la limite de sa réduction.
type Key = (PathBuf, Option<u32>);

/// Les images de la note, lues (et réduites) une seule fois.
#[derive(Default)]
pub(crate) struct ImageCache {
    memory: Mutex<HashMap<Key, (SystemTime, Bytes)>>,
    disk: Option<PathBuf>,
}

impl ImageCache {
    pub(crate) fn with_disk(dir: Option<PathBuf>) -> Self {
        if let Some(dir) = &dir {
            let dir = dir.clone();
            // Élagage en tâche de fond : le démarrage n'attend pas.
            std::thread::spawn(move || prune(&dir, DISK_BUDGET));
        }
        Self { memory: Mutex::default(), disk: dir }
    }

    /// Le contenu d'une image, réduite à `max` pixels si besoin.
    pub(crate) fn get(&self, path: &Path, max: Option<u32>) -> std::io::Result<Bytes> {
        let modified = std::fs::metadata(path)?.modified()?;
        let key = (path.to_path_buf(), max);
        if let Some((stamp, bytes)) = self.memory.lock().expect("cache d'images").get(&key)
            && *stamp == modified
        {
            return Ok(bytes.clone());
        }
        let bytes = self.load(path, max, modified)?;
        self.memory.lock().expect("cache d'images").insert(key, (modified, bytes.clone()));
        Ok(bytes)
    }

    /// Prépare d'avance les images à réduire, en parallèle (une par cœur),
    /// pour que la compilation les trouve prêtes.
    pub(crate) fn prepare(&self, items: &[(PathBuf, u32)]) {
        let todo: Vec<&(PathBuf, u32)> = {
            let memory = self.memory.lock().expect("cache d'images");
            items
                .iter()
                .filter(|(path, max)| {
                    let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok();
                    memory.get(&(path.clone(), Some(*max))).is_none_or(|(s, _)| Some(*s) != stamp)
                })
                .collect()
        };
        if todo.len() < 2 {
            return;
        }
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).min(todo.len());
        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((path, max)) = todo.get(index) else { break };
                        let _ = self.get(path, Some(*max));
                    }
                });
            }
        });
    }

    fn load(&self, path: &Path, max: Option<u32>, modified: SystemTime) -> std::io::Result<Bytes> {
        let raw = std::fs::read(path)?;
        let Some(max) = max else { return Ok(Bytes::new(raw)) };
        let cached =
            self.disk.as_ref().map(|dir| dir.join(format!("{:016x}.img", disk_key(path, &raw, max, modified))));
        if let Some(file) = &cached
            && let Ok(bytes) = std::fs::read(file)
        {
            return Ok(Bytes::new(bytes));
        }
        let original_len = raw.len();
        let out = downscale(raw, max);
        // Seules les photos réellement réduites valent d'être gardées.
        if out.len() < original_len
            && let Some(file) = &cached
        {
            let _ = write_atomically(file, &out);
        }
        Ok(Bytes::new(out))
    }
}

/// Clé stable d'une photo réduite : chemin, date, taille, limite (FNV-1a).
fn disk_key(path: &Path, raw: &[u8], max: u32, modified: SystemTime) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(path.to_string_lossy().as_bytes());
    feed(&(raw.len() as u64).to_le_bytes());
    let since = modified.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    feed(&since.to_le_bytes());
    feed(&max.to_le_bytes());
    feed(&[2]); // version de l'algorithme de réduction
    hash
}

fn write_atomically(file: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = file.with_extension("tmp");
    std::fs::write(&temp, data)?;
    std::fs::rename(&temp, file)
}

/// Garde le cache sur disque sous `budget` octets (les plus anciens partent).
fn prune(dir: &Path, budget: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= budget {
        return;
    }
    files.sort_by_key(|(time, ..)| *time);
    for (_, len, path) in files {
        if total <= budget * 3 / 4 {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, 120]));
        let mut jpeg = Vec::new();
        DynamicImage::ImageRgb8(img).write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg).unwrap();
        jpeg
    }

    #[test]
    fn large_photos_shrink_small_ones_stay() {
        let photo = jpeg(3000, 2000);
        let small = downscale(photo.clone(), 1200);
        let decoded = image::load_from_memory(&small).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (1200, 800));
        assert_eq!(downscale(photo.clone(), 4000), photo);
    }

    #[test]
    fn photos_are_prepared_once_and_kept_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = (0..3)
            .map(|i| {
                let path = dir.path().join(format!("photo{i}.jpg"));
                std::fs::write(&path, jpeg(2000 + i * 10, 1500)).unwrap();
                path
            })
            .collect();
        let disk = dir.path().join("cache");
        let cache = ImageCache::with_disk(Some(disk.clone()));
        cache.prepare(&paths.iter().map(|p| (p.clone(), 800)).collect::<Vec<_>>());
        assert_eq!(std::fs::read_dir(&disk).unwrap().count(), 3);
        // Une nouvelle session retrouve les photos réduites sur le disque.
        let again = ImageCache::with_disk(Some(disk));
        let bytes = again.get(&paths[0], Some(800)).unwrap();
        let decoded = image::load_from_memory(bytes.as_slice()).unwrap();
        assert_eq!(decoded.width(), 800);
    }
}
