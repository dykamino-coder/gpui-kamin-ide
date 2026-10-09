//! Language quotation marks from CLDR 46 delimiters, grouped by identical pairs.
//! CSS Content 3 §quotes-property; see Servo components/layout/quotes.rs data.

pub(super) fn marks(language: &str, depth: usize, open: bool) -> char {
    let language = language.to_ascii_lowercase();
    let parts: Vec<_> = language.split('-').collect();
    let primary = parts[0];
    // Prefer regional/script conventions, then fall back to the primary language.
    let regional = parts
        .iter()
        .skip(1)
        .find_map(|part| data(&format!("{primary}-{part}")));
    let marks = data(&language)
        .or(regional)
        .or_else(|| data(primary))
        .unwrap_or(['\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}']);
    marks[2 * depth.min(1) + usize::from(!open)]
}

fn data(language: &str) -> Option<[char; 4]> {
    Some(match language {
        "agq" | "ff" => ['\u{201e}', '\u{201d}', '\u{201a}', '\u{2019}'],
        "am" | "az-arab" | "az-cyrl" | "fa" | "fr-ch" | "gsw" | "jgo" | "kkj" | "mzn" | "sdh" => {
            ['\u{ab}', '\u{bb}', '\u{2039}', '\u{203a}']
        }
        "ar" | "lld" | "ms-arab" | "syr" | "ur" => ['\u{201d}', '\u{201c}', '\u{2019}', '\u{2018}'],
        "ast" | "blo" | "bm" | "br" | "ca" | "dyo" | "el" | "es-us" | "eu" | "ewo" | "ie"
        | "it" | "kab" | "kk" | "lij" | "mg" | "mua" | "nnh" | "pt-pt" | "sc" | "sg" | "sq"
        | "ti" => ['\u{ab}', '\u{bb}', '\u{201c}', '\u{201d}'],
        "bas" | "be" | "cv" | "ky" | "ru" | "sah" | "uk" => {
            ['\u{ab}', '\u{bb}', '\u{201e}', '\u{201c}']
        }
        "bg" | "lt" => ['\u{201e}', '\u{201c}', '\u{201e}', '\u{201c}'],
        "bs-cyrl" | "cs" | "de" | "dsb" | "et" | "hr" | "hsb" | "is" | "lb" | "luy" | "mk"
        | "sk" | "sl" => ['\u{201e}', '\u{201c}', '\u{201a}', '\u{2018}'],
        "bs" => ['\u{201e}', '\u{201d}', '\u{2018}', '\u{2019}'],
        "dua" | "el-polyton" | "ksf" | "no" | "rw" => ['\u{ab}', '\u{bb}', '\u{2018}', '\u{2019}'],
        "fi" | "he" | "lag" | "rn" | "sn" | "sv" => {
            ['\u{201d}', '\u{201d}', '\u{2019}', '\u{2019}']
        }
        "fr-ca" => ['\u{ab}', '\u{bb}', '\u{201d}', '\u{201c}'],
        "fr" | "hy" | "yav" => ['\u{ab}', '\u{bb}', '\u{ab}', '\u{bb}'],
        "hu" => ['\u{201e}', '\u{201d}', '\u{bb}', '\u{ab}'],
        "ia" | "nso" | "ti-er" | "tn" => ['\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}'],
        "ja" | "yue" | "zh-hant" => ['\u{300c}', '\u{300d}', '\u{300e}', '\u{300f}'],
        "ka" => ['\u{201e}', '\u{201c}', '\u{ab}', '\u{bb}'],
        "nl" => ['\u{2018}', '\u{2019}', '\u{2018}', '\u{2019}'],
        "nmg" | "pl" | "ro" => ['\u{201e}', '\u{201d}', '\u{ab}', '\u{bb}'],
        "shi" | "zgh" => ['\u{ab}', '\u{bb}', '\u{201e}', '\u{201d}'],
        "sr" => ['\u{201e}', '\u{201d}', '\u{2019}', '\u{2019}'],
        "st" => ['\u{201c}', '\u{2019}', '\u{201c}', '\u{201d}'],
        "tk" => ['\u{201c}', '\u{201d}', '\u{201c}', '\u{201d}'],
        "uz" => ['\u{201c}', '\u{201d}', '\u{2019}', '\u{2018}'],
        _ => return None,
    })
}
