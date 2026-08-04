//! Сборка таблиц ассетов: список SVG-иконок собирается ИЗ ПАПКИ, а не руками.
//!
//! `include_bytes!` требует литеральный путь, поэтому таблицу «путь → байты»
//! генерируем сюда, в `OUT_DIR`, и подключаем одним `include!`. Раньше те же
//! 620 записей лежали сгенерированным `.rs` на 2.5 тысячи строк и ломали
//! правило «файл ≤250 строк» без всякой пользы.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let assets = Path::new("assets/icons/cat");
    println!("cargo:rerun-if-changed={}", assets.display());

    let mut names: Vec<String> = std::fs::read_dir(assets)
        .unwrap_or_else(|e| panic!("нет папки {}: {e}", assets.display()))
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.ends_with(".svg").then_some(name)
        })
        .collect();
    names.sort();

    let mut out = String::from("// СГЕНЕРИРОВАНО build.rs — не редактировать.\n");
    out.push_str("pub const CAT_ICONS: &[(&str, &[u8])] = &[\n");
    for name in &names {
        let _ = writeln!(
            out,
            "    (\"icons/cat/{name}\", include_bytes!(r\"{}\")),",
            std::fs::canonicalize(assets.join(name))
                .expect("канонический путь ассета")
                .display()
                .to_string()
                .trim_start_matches(r"\\?\")
        );
    }
    out.push_str("];\n");

    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("cat_assets.rs");
    std::fs::write(&dest, out).expect("запись таблицы ассетов");

    // Ресурсы Windows: иконка и описание файла. Без них диспетчер задач
    // показывал пустой квадрат вместо иконки и голое имя exe вместо
    // «KaminIDE» (жалоба пользователя). Ресурс линкуется в ОБА бинарника
    // крейта — kaminide-web получает ту же иконку, что и главный.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/app/kaminide.ico");
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/app/kaminide.ico");
        res.set("FileDescription", "KaminIDE");
        res.set("ProductName", "KaminIDE");
        res.compile().expect("ресурсы Windows (иконка)");
    }
}
