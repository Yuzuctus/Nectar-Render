//! Frontmatter YAML d'une note Obsidian.

use yaml_rust2::{Yaml, YamlLoader};

use crate::model::Meta;

/// Lit le bloc `---` … `---` (délimiteurs compris).
pub fn parse(block: &str, warnings: &mut Vec<String>) -> Meta {
    let body: String = block.lines().filter(|line| line.trim_end() != "---").collect::<Vec<_>>().join("\n");
    let docs = match YamlLoader::load_from_str(&body) {
        Ok(docs) => docs,
        Err(e) => {
            warnings.push(format!("frontmatter illisible : {e}"));
            return Meta::default();
        }
    };
    let Some(Yaml::Hash(map)) = docs.into_iter().next() else { return Meta::default() };
    let get = |key: &str| map.get(&Yaml::String(key.into()));
    Meta {
        title: get("title").and_then(scalar),
        subtitle: get("subtitle").or_else(|| get("description")).and_then(scalar),
        author: get("author").or_else(|| get("authors")).and_then(joined),
        date: get("date").and_then(scalar),
        lang: get("lang").or_else(|| get("language")).and_then(scalar),
        tags: get("tags").map(list).unwrap_or_default(),
    }
}

fn scalar(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(s) | Yaml::Real(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

fn list(value: &Yaml) -> Vec<String> {
    match value {
        Yaml::Array(items) => items.iter().filter_map(scalar).collect(),
        other => scalar(other)
            .map(|s| s.split([',', ' ']).filter(|t| !t.is_empty()).map(str::to_string).collect())
            .unwrap_or_default(),
    }
}

fn joined(value: &Yaml) -> Option<String> {
    let items = list(value);
    match value {
        Yaml::Array(_) if !items.is_empty() => Some(items.join(", ")),
        _ => scalar(value),
    }
}
