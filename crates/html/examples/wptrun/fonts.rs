//! Регистрация служебных шрифтов WPT до измерений документов.

pub(super) fn register(cx: &gpui::App) {
    // Ahem — служебный шрифт набора: все его буквы одинаковые чёрные
    // квадраты в кегль. На нём построены сотни тестов: фигуры сходятся
    // ровно потому, что метрики предсказуемы. Без него текст набирается
    // системным шрифтом, и «квадраты» разъезжаются.
    for name in ["Ahem.ttf", "AHEM____.TTF"] {
        let path = std::path::Path::new("vendor/wpt-parsing/fonts").join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            let _ = cx.text_system().add_fonts(vec![bytes.into()]);
            break;
        }
    }
    // Шрифты CSSTest — второе предусловие набора: тесты css-fonts пишут
    // «Test fonts must be installed for this test» и зовут их по имени
    // семейства как УСТАНОВЛЕННЫЕ (`font-family: CSSTest Verify`), без
    // `@font-face`. Без них обе стороны пары набираются подменой.
    if let Ok(dir) = std::fs::read_dir("vendor/wpt-parsing/fonts/CSSTest") {
        let mut paths: Vec<_> = dir.flatten().map(|e| e.path()).collect();
        paths.sort();
        let fonts: Vec<_> = paths
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "ttf"))
            .filter_map(|p| std::fs::read(p).ok())
            .map(Into::into)
            .collect();
        let _ = cx.text_system().add_fonts(fonts);
    }
}
