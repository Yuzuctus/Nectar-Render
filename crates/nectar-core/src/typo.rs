//! Typographie française : espaces insécables devant la ponctuation haute et
//! à l'intérieur des guillemets, pour qu'un « ! » ne parte jamais seul à la
//! ligne.
//!
//! - `; ! ?` et `»` : espace fine insécable (U+202F) ;
//! - `:` : espace insécable normale (U+00A0) ;
//! - `«` : suivi d'une espace fine insécable.
//!
//! Les cas qui ne sont pas de la ponctuation de phrase restent intacts :
//! heures (`12:30`), adresses (`https://…`, `page.php?id=2`).

const NARROW: char = '\u{202F}';
const NBSP: char = '\u{00A0}';

/// Applique les règles à un morceau de texte. `after_word` indique que le
/// texte suit directement un mot (fin d'un passage en gras, par exemple).
pub fn french(text: &str, after_word: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        let next = chars.get(i + 1).copied();
        let ends_phrase = next.is_none_or(|n| n.is_whitespace() || is_punct(n) || n == '»' || n == '"');
        match c {
            ';' | '!' | '?' if ends_phrase => {
                let had_space = trim_spaces(&mut out);
                let prev = out.last().copied();
                let attach = match prev {
                    Some(';' | '!' | '?' | NARROW | NBSP) => false,
                    Some(p) => had_space || p.is_alphanumeric() || matches!(p, '»' | ')' | ']' | '"' | '’' | '%' | '…'),
                    None => had_space || after_word,
                };
                if attach {
                    out.push(NARROW);
                } else if had_space && prev.is_none() {
                    out.push(' ');
                }
                out.push(c);
            }
            ':' => {
                let prev = out.last().copied();
                let time = prev.is_some_and(|p| p.is_ascii_digit()) && next.is_some_and(|n| n.is_ascii_digit());
                let url = next == Some('/');
                if time || url {
                    out.push(c);
                    continue;
                }
                let had_space = trim_spaces(&mut out);
                let prev = out.last().copied();
                let attach = match prev {
                    Some(p) if p == NBSP || p == NARROW => false,
                    Some(p) => had_space || (p.is_alphabetic() && next.is_none_or(char::is_whitespace)),
                    None => had_space || (after_word && next.is_none_or(char::is_whitespace)),
                };
                if attach {
                    out.push(NBSP);
                } else if had_space {
                    out.push(' ');
                }
                out.push(c);
            }
            '«' => {
                out.push(c);
                if next.is_some_and(|n| n != NARROW && n != NBSP) {
                    out.push(NARROW);
                }
            }
            '»' => {
                trim_spaces(&mut out);
                if out.last().is_some_and(|&p| p != NARROW && p != NBSP) {
                    out.push(NARROW);
                }
                out.push(c);
            }
            // Espaces juste après « : déjà remplacées par l'espace fine.
            ' ' if out.last() == Some(&NARROW) && out.len() >= 2 && out[out.len() - 2] == '«' => {}
            _ => out.push(c),
        }
    }
    out.into_iter().collect()
}

fn is_punct(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '…')
}

/// Retire les espaces ordinaires en fin de sortie ; dit s'il y en avait.
fn trim_spaces(out: &mut Vec<char>) -> bool {
    let mut had = false;
    while out.last().is_some_and(|&c| c == ' ' || c == '\t') {
        out.pop();
        had = true;
    }
    had
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(s: &str) -> String {
        french(s, false).replace(NARROW, "⍽").replace(NBSP, "_")
    }

    #[test]
    fn high_punctuation_gets_narrow_spaces() {
        assert_eq!(f("Vraiment ? Oui ! Bon ; voilà"), "Vraiment⍽? Oui⍽! Bon⍽; voilà");
        assert_eq!(f("Vraiment?! Oui!"), "Vraiment⍽?! Oui⍽!");
        assert_eq!(f("« déjà » et «cité»"), "«⍽déjà⍽» et «⍽cité⍽»");
    }

    #[test]
    fn colon_gets_a_regular_nbsp() {
        assert_eq!(f("Note : texte"), "Note_: texte");
        assert_eq!(f("Note: texte"), "Note_: texte");
    }

    #[test]
    fn leaves_times_urls_and_queries_alone() {
        assert_eq!(f("à 12:30 sur https://ex.com/a?b=1"), "à 12:30 sur https://ex.com/a?b=1");
        assert_eq!(f("index.php?id=2"), "index.php?id=2");
    }

    #[test]
    fn punctuation_after_formatted_word() {
        // « **gras** ! » : le texte commence par l'espace et le point d'exclamation.
        assert_eq!(f(" !"), "⍽!");
        assert_eq!(french("!", true), "\u{202F}!");
        assert_eq!(french("!", false), "!");
    }
}
