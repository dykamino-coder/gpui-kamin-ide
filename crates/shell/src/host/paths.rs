//! Пути данных: dev-репо и локальные каталоги.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use std::path::PathBuf;

/// Корень node-части (kamin-host/extensions) для dev-режима. С миграции
/// 2026-08-03 живёт В ЭТОМ ЖЕ репо — kamin-ide мёртв и не участвует.
/// Фолбэк — текущий каталог (dev-запуск идёт из корня репо); захардкоженный
/// путь машины разработчика утекал в строки release-бинаря.
pub(crate) fn dev_repo() -> PathBuf {
    std::env::var("KAMIN_DEV_REPO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// НАШИ данные — не трогаем прод studio.dykamino.kaminide.
pub fn data_dirs() -> (PathBuf, PathBuf) {
    let base = dirs_local().join("kaminide-gpui-dev");
    (base.join("data"), base.join("cache"))
}

pub(crate) fn dirs_local() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
}
