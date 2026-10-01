//! Photos trop lourdes : réduites avant d'entrer dans le PDF.
//!
//! Une photo de téléphone (4000 × 3000 px) imprimée sur une demi-page n'a
//! besoin que d'environ 1800 px. Au-delà de la limite, on la réduit
//! (Lanczos) et on la réencode : JPEG qualité 88, PNG sans perte.

use std::io::Cursor;

use image::{ImageFormat, ImageReader};

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
    let Ok(img) = image::load_from_memory_with_format(&bytes, format) else { return bytes };
    let resized = img.resize(max, max, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::new();
    let encoded = match format {
        ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
            resized.to_rgb8().write_with_encoder(encoder)
        }
        _ => resized.write_to(&mut Cursor::new(&mut out), ImageFormat::Png),
    };
    if encoded.is_ok() && out.len() < bytes.len() { out } else { bytes }
}

#[cfg(test)]
mod tests {
    #[test]
    fn large_photos_shrink_small_ones_stay() {
        let img = image::RgbImage::from_fn(3000, 2000, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, 120]));
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        let small = super::downscale(jpeg.clone(), 1200);
        let decoded = image::load_from_memory(&small).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (1200, 800));
        assert_eq!(super::downscale(jpeg.clone(), 4000), jpeg);
    }
}
