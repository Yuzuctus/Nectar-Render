//! Le journal des erreurs : `%LOCALAPPDATA%\Nectar Render\journal.txt`.
//!
//! Les erreurs du moteur et les plantages y sont notés avec leur date, pour
//! pouvoir les retrouver (et les transmettre) après coup. Le fichier reste
//! petit : au-delà de 512 Ko, seule la fin est gardée.

use std::io::Write as _;
use std::path::PathBuf;

const LIMIT: u64 = 512 * 1024;

fn path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_STATE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))
        .map(|base| base.join("Nectar Render").join("journal.txt"))
}

/// Ajoute une ligne datée au journal (sans jamais échouer).
pub fn write(message: &str) {
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LIMIT)
        && let Ok(text) = std::fs::read_to_string(&path)
    {
        let keep = text.len() / 2;
        let start = text.char_indices().map(|(i, _)| i).find(|i| *i >= keep).unwrap_or(0);
        let _ = std::fs::write(&path, &text[start..]);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "[{}] {}", now(), message.trim_end());
    }
}

/// Note dans le journal tout plantage, avant le comportement habituel.
pub fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let place = info.location().map(|l| format!(" ({}:{})", l.file(), l.line())).unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| info.payload().downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("principal").to_string();
        write(&format!("plantage dans « {thread} »{place} : {message}"));
        default(info);
    }));
}

/// Date et heure (UTC) lisibles, sans dépendance.
fn now() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Jours depuis 1970 → date civile (algorithme de Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC", rest / 3600, rest % 3600 / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn date_is_readable() {
        let now = super::now();
        assert!(now.len() == 23 && now.ends_with(" UTC") && now.starts_with("20"), "{now}");
    }
}
