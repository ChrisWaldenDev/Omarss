//! Generated TypeScript bindings for every command and IPC type (SPEC §4.3).
//! Only compiled into debug and test builds; release builds never touch the source tree.

use std::path::Path;

use specta_typescript::Typescript;

pub const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/types.ts");

/// Writes the TypeScript bindings to `path`, touching the file only if its content changed
/// (so the Vite dev server doesn't reload for nothing).
pub fn export(builder: &tauri_specta::Builder<tauri::Wry>, path: &Path) -> Result<(), String> {
    let tmp = std::env::temp_dir().join(format!("omarss-bindings-{}.ts", std::process::id()));
    builder
        .export(Typescript::default(), &tmp)
        .map_err(|err| err.to_string())?;
    let fresh = std::fs::read_to_string(&tmp).map_err(|err| err.to_string());
    let _ = std::fs::remove_file(&tmp);
    let fresh = fresh?;
    if std::fs::read_to_string(path).ok().as_deref() != Some(fresh.as_str()) {
        std::fs::write(path, fresh).map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates `src/lib/types.ts`. CI fails if this leaves the file changed, i.e. if the
    /// committed bindings are stale.
    #[test]
    fn export_typescript_bindings() {
        export(&crate::specta_builder(), Path::new(BINDINGS_PATH)).unwrap();
        let ts = std::fs::read_to_string(BINDINGS_PATH).unwrap();
        for name in [
            "getSettings",
            "updateSettings",
            "listArticles",
            "openExternal",
        ] {
            assert!(ts.contains(name), "bindings are missing {name}");
        }
    }
}
