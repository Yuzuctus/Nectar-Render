//! Le HTML qu'on croise dans les notes Obsidian : quelques balises de mise
//! en forme et des images dimensionnées. Pas un moteur HTML.

use crate::model::{Inline, plain_text};

/// Valeur d'un attribut (`src="x"`, `src='x'` ou `src=x`).
pub fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut search = 0;
    while let Some(found) = lower[search..].find(name) {
        let start = search + found;
        search = start + name.len();
        let before = lower[..start].chars().last();
        if before.is_some_and(|c| !c.is_whitespace()) {
            continue;
        }
        let rest = tag[start + name.len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else { continue };
        let rest = rest.trim_start();
        let value = match rest.chars().next()? {
            q @ ('"' | '\'') => rest[1..].split(q).next()?.to_string(),
            _ => rest.split(|c: char| c.is_whitespace() || c == '>' || c == '/').next()?.to_string(),
        };
        return Some(decode(&value));
    }
    None
}

/// `300`, `300px` → 300 ; les pourcentages sont ignorés.
pub fn pixels(value: &str) -> Option<u32> {
    value.trim().trim_end_matches("px").trim().parse::<f32>().ok().map(|v| v.round() as u32)
}

/// Toutes les balises ouvrantes d'un nom donné.
pub fn tags(html: &str, name: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let needle = format!("<{name}");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find(&needle) {
        let start = from + i;
        let end = html[start..].find('>').map(|e| start + e + 1).unwrap_or(html.len());
        out.push(html[start..end].to_string());
        from = end;
    }
    out
}

/// Le bloc demande-t-il un centrage (`align="center"`, `<center>`, `text-align: center`) ?
pub fn centered(html: &str) -> bool {
    let lower = html.to_ascii_lowercase().replace(' ', "");
    lower.contains("<center")
        || lower.contains("align=\"center\"")
        || lower.contains("align='center'")
        || lower.contains("align=center")
        || lower.contains("text-align:center")
}

/// Texte d'un fragment HTML, balises retirées.
pub fn strip(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode(&out)
}

fn decode(text: &str) -> String {
    text.replace("&nbsp;", "\u{a0}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Nom d'une balise et si elle ferme : `</b>` → ("b", true).
fn tag_name(tag: &str) -> (String, bool) {
    let inner = tag.trim_start_matches('<').trim_end_matches('>').trim();
    let closing = inner.starts_with('/');
    let name = inner.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
    (name.to_ascii_lowercase(), closing)
}

/// Apparie les balises ouvrantes et fermantes et les transforme en mise en
/// forme. Les balises inconnues sont transparentes (leur contenu reste).
pub fn fold(items: Vec<Inline>) -> Vec<Inline> {
    if !items.iter().any(|i| matches!(i, Inline::Html(_))) {
        return items;
    }
    // Pile de (nom, balise complète, contenu).
    let mut stack: Vec<(String, String, Vec<Inline>)> = vec![(String::new(), String::new(), Vec::new())];
    for item in items {
        match item {
            Inline::Html(tag) => {
                let (name, closing) = tag_name(&tag);
                if !closing {
                    if !tag.ends_with("/>") {
                        stack.push((name, tag, Vec::new()));
                    }
                    continue;
                }
                let Some(depth) = stack.iter().rposition(|(n, _, _)| *n == name).filter(|d| *d > 0) else { continue };
                // Les balises restées ouvertes à l'intérieur se dissolvent.
                while stack.len() > depth + 1 {
                    let (_, _, content) = stack.pop().expect("pile");
                    stack.last_mut().expect("pile").2.extend(content);
                }
                let (name, open, content) = stack.pop().expect("pile");
                let wrapped = wrap(&name, &open, content);
                stack.last_mut().expect("pile").2.extend(wrapped);
            }
            other => stack.last_mut().expect("pile").2.push(other),
        }
    }
    while stack.len() > 1 {
        let (_, _, content) = stack.pop().expect("pile");
        stack.last_mut().expect("pile").2.extend(content);
    }
    stack.pop().map(|(_, _, c)| c).unwrap_or_default()
}

fn wrap(name: &str, open: &str, content: Vec<Inline>) -> Vec<Inline> {
    let one = |i: Inline| vec![i];
    match name {
        "b" | "strong" => one(Inline::Strong(content)),
        "i" | "em" | "cite" | "var" => one(Inline::Emph(content)),
        "u" | "ins" => one(Inline::Underline(content)),
        "s" | "del" | "strike" => one(Inline::Strike(content)),
        "mark" => one(Inline::Highlight(content)),
        "sup" => one(Inline::Superscript(content)),
        "sub" => one(Inline::Subscript(content)),
        "kbd" => one(Inline::Kbd(plain_text(&content))),
        "code" | "samp" => one(Inline::Code(plain_text(&content))),
        "a" => match attr(open, "href") {
            Some(url) => one(Inline::Link { url, content }),
            None => content,
        },
        _ => content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes() {
        let tag = r#"<img alt='un "chat"' src="a b.png" width=300px>"#;
        assert_eq!(attr(tag, "src").as_deref(), Some("a b.png"));
        assert_eq!(attr(tag, "alt").as_deref(), Some("un \"chat\""));
        assert_eq!(attr(tag, "width").and_then(|w| pixels(&w)), Some(300));
        assert_eq!(attr(r#"<img data-src="x" src="y">"#, "src").as_deref(), Some("y"));
    }
}
