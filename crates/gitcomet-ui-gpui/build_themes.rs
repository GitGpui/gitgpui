use std::env;
use std::fs;
use std::path::PathBuf;

pub fn generate_embedded_theme_registry() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let themes_dir = manifest_dir.join("assets/themes");
    println!("cargo:rerun-if-changed={}", themes_dir.display());

    let mut theme_files = fs::read_dir(&themes_dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", themes_dir.display()))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("json")).then_some(path)
        })
        .collect::<Vec<_>>();

    theme_files.sort();

    let mut generated = String::from("static EMBEDDED_THEME_SPECS: &[EmbeddedThemeSpec] = &[\n");
    for path in theme_files {
        println!("cargo:rerun-if-changed={}", path.display());
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| panic!("invalid theme file name: {}", path.display()));
        let key = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_else(|| panic!("invalid theme file stem: {}", path.display()));
        let label = humanize_theme_key(key);

        generated.push_str(&format!(
            "    EmbeddedThemeSpec {{ key: {key:?}, label: {label:?}, json: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/themes/{file_name}\")) }},\n"
        ));
    }
    generated.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("embedded_themes.rs"), generated)
        .unwrap_or_else(|err| panic!("failed to write embedded theme registry: {err}"));
}

fn humanize_theme_key(key: &str) -> String {
    key.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            if part.eq_ignore_ascii_case("gitcomet") {
                return "GitComet".to_string();
            }
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = String::new();
                    word.push(first.to_ascii_uppercase());
                    word.extend(chars.map(|ch| ch.to_ascii_lowercase()));
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
