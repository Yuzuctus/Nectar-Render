//! Brancher Nectar à une IA.
//!
//! Une IA ne découvre pas seule les serveurs MCP : Claude Desktop les lit
//! dans son fichier de réglages (`claude_desktop_config.json`), Claude Code
//! dans les siens (`claude mcp add`). On écrit la ligne de Nectar à sa place,
//! sans toucher aux autres serveurs, et en gardant une copie de l'ancien
//! fichier.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// Nom du serveur dans les réglages de l'IA.
pub const SERVER_NAME: &str = "nectar-render";

/// Les fichiers de réglages de Claude Desktop présents sur la machine (ou,
/// s'il n'y en a aucun, l'emplacement habituel).
pub fn claude_desktop_configs() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut usual = None;
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let path = PathBuf::from(appdata).join("Claude").join("claude_desktop_config.json");
        usual = Some(path.clone());
        found.push(path);
        // Version du Microsoft Store : ses réglages vivent dans son paquet.
        if let Some(local) = std::env::var_os("LOCALAPPDATA")
            && let Ok(entries) = std::fs::read_dir(PathBuf::from(local).join("Packages"))
        {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with("Claude_") {
                    found.push(
                        entry
                            .path()
                            .join("LocalCache")
                            .join("Roaming")
                            .join("Claude")
                            .join("claude_desktop_config.json"),
                    );
                }
            }
        }
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let mac = home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json");
        let linux = home.join(".config").join("Claude").join("claude_desktop_config.json");
        usual = Some(if cfg!(target_os = "macos") { mac.clone() } else { linux.clone() });
        found.extend([mac, linux]);
    }
    // Seuls les dossiers où Claude Desktop est installé comptent.
    found.retain(|p| p.parent().is_some_and(Path::is_dir));
    if found.is_empty() {
        found.extend(usual);
    }
    found
}

/// Ajoute (ou met à jour) Nectar dans un fichier de réglages de Claude
/// Desktop. Un fichier illisible n'est jamais écrasé.
pub fn connect_claude_desktop_at(config: &Path, nectar: &Path) -> Result<(), String> {
    let mut root = match std::fs::read_to_string(config) {
        Ok(text) if text.trim().is_empty() => Value::Object(Map::new()),
        Ok(text) => serde_json::from_str::<Value>(&text).map_err(|e| {
            format!("{} n'est pas un JSON valide ({e}) : corrigez-le ou ajoutez Nectar à la main.", config.display())
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(Map::new()),
        Err(e) => return Err(format!("lecture de {} impossible : {e}", config.display())),
    };
    let Some(object) = root.as_object_mut() else {
        return Err(format!("{} ne contient pas un objet JSON.", config.display()));
    };
    let servers = object.entry("mcpServers").or_insert_with(|| Value::Object(Map::new()));
    let Some(servers) = servers.as_object_mut() else {
        return Err(format!("« mcpServers » de {} n'est pas un objet.", config.display()));
    };
    servers.insert(SERVER_NAME.into(), json!({ "command": nectar.display().to_string(), "args": ["mcp"] }));
    if config.exists() {
        let backup = config.with_extension("json.bak");
        std::fs::copy(config, &backup).map_err(|e| format!("copie de sauvegarde impossible : {e}"))?;
    }
    if let Some(dir) = config.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("création de {} impossible : {e}", dir.display()))?;
    }
    let mut text = serde_json::to_string_pretty(&root).expect("JSON sérialisable");
    text.push('\n');
    crate::write_atomically(config, text.as_bytes())
        .map_err(|e| format!("écriture de {} impossible : {e}", config.display()))
}

/// Branche Nectar à Claude Desktop ; rend les fichiers modifiés.
pub fn connect_claude_desktop(nectar: &Path) -> Result<Vec<PathBuf>, String> {
    let configs = claude_desktop_configs();
    if configs.is_empty() {
        return Err("emplacement des réglages de Claude Desktop inconnu sur cette machine".into());
    }
    for config in &configs {
        connect_claude_desktop_at(config, nectar)?;
    }
    Ok(configs)
}

/// Nectar est-il déjà branché à Claude Desktop (vers ce `nectar`) ?
pub fn claude_desktop_connected(nectar: &Path) -> bool {
    claude_desktop_configs().iter().any(|config| {
        std::fs::read_to_string(config)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|root| root.get("mcpServers")?.get(SERVER_NAME)?.get("command")?.as_str().map(PathBuf::from))
            .is_some_and(|command| command == nectar)
    })
}

/// La commande qui branche Nectar à Claude Code.
pub fn claude_code_command(nectar: &Path) -> String {
    format!("claude mcp add {SERVER_NAME} --scope user -- \"{}\" mcp", nectar.display())
}

/// `nectar.exe` (ou `nectar`) à côté de l'exécutable en cours.
pub fn nectar_beside_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let name = if cfg!(windows) { "nectar.exe" } else { "nectar" };
    let path = exe.parent()?.join(name);
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_nectar_and_keeps_other_servers() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("Claude").join("claude_desktop_config.json");
        let nectar = Path::new("C:/Programmes/Nectar Render/nectar.exe");
        // Pas encore de fichier : il est créé.
        connect_claude_desktop_at(&config, nectar).unwrap();
        // Un autre serveur et un autre réglage restent en place.
        let mut root: Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        root["mcpServers"]["autre"] = json!({ "command": "autre.exe" });
        root["theme"] = json!("dark");
        std::fs::write(&config, serde_json::to_string(&root).unwrap()).unwrap();
        connect_claude_desktop_at(&config, nectar).unwrap();
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(root["mcpServers"][SERVER_NAME]["args"], json!(["mcp"]));
        assert_eq!(root["mcpServers"][SERVER_NAME]["command"], json!(nectar.display().to_string()));
        assert_eq!(root["mcpServers"]["autre"]["command"], json!("autre.exe"));
        assert_eq!(root["theme"], json!("dark"));
        assert!(config.with_extension("json.bak").exists(), "copie de l'ancien fichier");
    }

    #[test]
    fn never_overwrites_a_broken_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("claude_desktop_config.json");
        std::fs::write(&config, "{ pas du json").unwrap();
        assert!(connect_claude_desktop_at(&config, Path::new("nectar.exe")).is_err());
        assert_eq!(std::fs::read_to_string(&config).unwrap(), "{ pas du json");
    }
}
