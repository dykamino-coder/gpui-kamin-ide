//! Наследуемые длинные свойства реестра вне шрифта и текста шага 6b (и
//! ненаследуемые части их сокращений: `text-box-trim`, `max-lines`,
//! `continue`): списки и кавычки (css-lists-3, css-content-3), письмо
//! (css-writing-modes-4, MathML Core), цвет (css-color-4), взаимодействие
//! (css-ui-4, css-scrollbars-1, css-images-3, css-color-adjust-1), SVG (css-fill-stroke-3),
//! таблицы и фрагментация (CSS 2.1 §17, css-break-3), строчный текст
//! (css-text-4, css-text-decor-4, css-ruby-1, css-inline-3, css-overflow-4).
//! Строки дописываются к таблице `longhands!` вызовом с продолжением.

macro_rules! with_inherited_rows {
    ($($rows:tt)*) => {
        longhands! {
            $($rows)*
        ListStyleType = "list-style-type", "disc", true, List, N;
        ListStylePosition = "list-style-position", "outside", true, List, N;
        ListStyleImage = "list-style-image", "none", true, List, N;
        Quotes = "quotes", "auto", true, List, N;
        Direction = "direction", "ltr", true, Writing, N;
        WritingMode = "writing-mode", "horizontal-tb", true, Writing, N;
        TextOrientation = "text-orientation", "mixed", true, Writing, N;
        TextCombineUpright = "text-combine-upright", "none", true, Writing, N;
        MathDepth = "math-depth", "0", true, Writing, N;
        MathShift = "math-shift", "normal", true, Writing, N;
        MathStyle = "math-style", "normal", true, Writing, N;
        Color = "color", "canvastext", true, Color, N;
        ColorScheme = "color-scheme", "normal", true, Color, N;
        Cursor = "cursor", "auto", true, Ui, N;
        PointerEvents = "pointer-events", "auto", true, Ui, N;
        Visibility = "visibility", "visible", true, Ui, N;
        AccentColor = "accent-color", "auto", true, Ui, N;
        CaretColor = "caret-color", "auto", true, Ui, N;
        ScrollbarColor = "scrollbar-color", "auto", true, Ui, N;
        ImageRendering = "image-rendering", "auto", true, Ui, N;
        ImageOrientation = "image-orientation", "from-image", true, Ui, N;
        Fill = "fill", "black", true, Svg, N;
        Stroke = "stroke", "none", true, Svg, N;
        StrokeWidth = "stroke-width", "1px", true, Svg, N;
        BorderCollapse = "border-collapse", "separate", true, Table, N;
        BorderSpacing = "border-spacing", "0px", true, Table, N;
        CaptionSide = "caption-side", "top", true, Table, N;
        EmptyCells = "empty-cells", "show", true, Table, N;
        Orphans = "orphans", "2", true, Table, N;
        Widows = "widows", "2", true, Table, N;
        LetterSpacing = "letter-spacing", "normal", true, Inline, N;
        WordSpacing = "word-spacing", "normal", true, Inline, N;
        TextIndent = "text-indent", "0", true, Inline, N;
        TextAlignAll = "text-align-all", "start", true, Inline, N;
        TextAlignLast = "text-align-last", "auto", true, Inline, N;
        TextJustify = "text-justify", "auto", true, Inline, N;
        TextTransform = "text-transform", "none", true, Inline, N;
        TabSize = "tab-size", "8", true, Inline, N;
        OverflowWrap = "overflow-wrap", "normal", true, Inline, N;
        WordBreak = "word-break", "normal", true, Inline, N;
        LineBreak = "line-break", "auto", true, Inline, N;
        Hyphens = "hyphens", "manual", true, Inline, N;
        HyphenateCharacter = "hyphenate-character", "auto", true, Inline, N;
        HyphenateLimitChars = "hyphenate-limit-chars", "auto", true, Inline, N;
        HyphenateLimitLines = "hyphenate-limit-lines", "no-limit", true, Inline, N;
        WordSpaceTransform = "word-space-transform", "none", true, Inline, N;
        TextAutospace = "text-autospace", "normal", true, Inline, N;
        HangingPunctuation = "hanging-punctuation", "none", true, Inline, N;
        TextFit = "text-fit", "none", true, Inline, N;
        TextBoxTrim = "text-box-trim", "none", false, Inline, N;
        TextBoxEdge = "text-box-edge", "auto", true, Inline, N;
        MaxLines = "max-lines", "auto", false, Inline, N;
        BlockEllipsis = "block-ellipsis", "no-ellipsis", true, Inline, N;
        Continue = "continue", "normal", false, Inline, N;
        TextShadow = "text-shadow", "none", true, Inline, N;
        TextUnderlineOffset = "text-underline-offset", "auto", true, Inline, N;
        TextUnderlinePosition = "text-underline-position", "auto", true, Inline, N;
        TextDecorationSkipInk = "text-decoration-skip-ink", "auto", true, Inline, N;
        TextDecorationSkipSpaces = "text-decoration-skip-spaces", "start end", true, Inline, N;
        TextEmphasisPosition = "text-emphasis-position", "over right", true, Inline, N;
        RubyPosition = "ruby-position", "alternate", true, Inline, N;
        RubyAlign = "ruby-align", "space-around", true, Inline, N;
        RubyOverhang = "ruby-overhang", "auto", true, Inline, N;
        RubyMerge = "ruby-merge", "separate", true, Inline, N;
        }
    };
}
