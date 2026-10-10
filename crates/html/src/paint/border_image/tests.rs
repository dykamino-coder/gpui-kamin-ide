//! Проверки контракта родительского модуля; вынесены для ограничения размера файлов.

/// Вырезка с растяжением: угловой знак образа приходит чистым, без
/// подмешивания соседей.
#[test]
fn crop_keeps_the_corner_colour() {
    use image::Frame;
    // 2×2: (0,0) зелёный, остальные красные. Байты в порядке BGRA.
    let g = [0u8, 128, 0, 255];
    let r = [0u8, 0, 255, 255];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&g);
    bytes.extend_from_slice(&r);
    bytes.extend_from_slice(&r);
    bytes.extend_from_slice(&r);
    let buffer = image::RgbaImage::from_raw(2, 2, bytes).unwrap();
    let image = gpui::RenderImage::new(smallvec::SmallVec::<[Frame; 1]>::from_elem(
        Frame::new(buffer),
        1,
    ));
    let cut = gpui::crop_image(&image, 0, 0, 1, 1, 4, 4).expect("вырезка");
    let out = cut.as_bytes(0).unwrap();
    assert_eq!(out.len(), 4 * 4 * 4);
    for px in out.chunks_exact(4) {
        assert_eq!(px, &g, "весь кусок — цвет углового знака");
    }
}
