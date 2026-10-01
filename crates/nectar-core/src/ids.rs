//! Identifiants stables des blocs.
//!
//! Un id vaut `<type>-<empreinte>` où l'empreinte est un FNV-1a 32 bits du
//! texte normalisé. Il ne dépend ni de la position ni des lignes vides : on
//! peut déplacer un paragraphe sans perdre ses retouches. Deux blocs au texte
//! identique reçoivent un suffixe d'occurrence (`p-1a2b3c4d-2`).

use std::collections::HashMap;

use crate::model::{BlockId, BlockKind};

/// Normalise un texte pour l'empreinte : minuscules, espaces compactés.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.extend(word.chars().flat_map(char::to_lowercase));
    }
    out
}

/// FNV-1a 32 bits : stable d'une version de Rust à l'autre.
fn fnv1a(text: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in text.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// Début de texte lisible, utilisé comme indice pour l'ancrage approximatif.
pub fn excerpt(text: &str, max_chars: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(max_chars) {
        Some((cut, _)) => format!("{}…", flat[..cut].trim_end()),
        None => flat,
    }
}

/// Distribue des ids uniques au fil de la lecture du document.
#[derive(Default)]
pub struct IdAllocator {
    seen: HashMap<String, usize>,
}

impl IdAllocator {
    pub fn allocate(&mut self, kind: BlockKind, text: &str) -> BlockId {
        let base = format!("{}-{:08x}", kind.prefix(), fnv1a(&normalize(text)));
        let count = self.seen.entry(base.clone()).or_insert(0);
        *count += 1;
        if *count == 1 { BlockId(base) } else { BlockId(format!("{base}-{count}")) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_ignore_whitespace_and_case() {
        let mut a = IdAllocator::default();
        let mut b = IdAllocator::default();
        assert_eq!(
            a.allocate(BlockKind::Paragraph, "Une  phrase\nici"),
            b.allocate(BlockKind::Paragraph, "une phrase ici"),
        );
    }

    #[test]
    fn duplicates_get_a_suffix() {
        let mut ids = IdAllocator::default();
        let first = ids.allocate(BlockKind::Paragraph, "même texte");
        let second = ids.allocate(BlockKind::Paragraph, "même texte");
        assert_eq!(second.0, format!("{}-2", first.0));
    }

    #[test]
    fn excerpt_cuts_on_char_boundary() {
        assert_eq!(excerpt("été à la plage", 3), "été…");
        assert_eq!(excerpt("court", 10), "court");
    }
}
