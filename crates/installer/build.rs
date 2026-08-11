//! Ресурсы Windows для инсталлера: иконка и поля версии.
//!
//! Без них Проводник рисует у `KaminIDE_<ver>_x64-setup.exe` пустой лист вместо
//! логотипа, а «Свойства» показывают голое имя файла. У NSIS иконка бралась из
//! его собственной директивы — при переезде на свой распаковщик она пропала.
//! Иконка та же, что у шелла: файл лежит в его крейте, второй копии не заводим.

fn main() {
    #[cfg(windows)]
    {
        let icon = "../shell/assets/app/kaminide.ico";
        println!("cargo:rerun-if-changed={icon}");
        let mut res = winres::WindowsResource::new();
        res.set_icon(icon);
        res.set("FileDescription", "KaminIDE — установка");
        res.set("ProductName", "KaminIDE");
        res.compile().expect("ресурсы Windows (иконка инсталлера)");
    }
}
