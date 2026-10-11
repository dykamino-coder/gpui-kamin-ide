//! Шторка переключения: deadline и fade не зависят от декоративной анимации.

/// Шаг шторки переключения чата за кадр: `None` — шторки нет.
///
/// Отметка форс-снятия ОБЯЗАТЕЛЬНО записывается обратно в состояние. Раньше
/// она вычислялась локально (`and_then` берёт кортеж по значению), поэтому на
/// каждом кадре отсчёт затухания начинался заново: прозрачность оставалась
/// 1.0, и шторка висела вечно, если подтверждение готовности не пришло
/// (INC-2026-0005). Поэтому функция принимает состояние по `&mut` — сохранение
/// отметки и есть суть исправления, и тест обязан проверять именно его.
pub(super) fn cover_step(
    cover: &mut Option<(std::time::Instant, Option<std::time::Instant>)>,
    timeout_ms: u128,
    grace_ms: u128,
    fade_ms: f32,
) -> Option<f32> {
    let (started, cleared) = (*cover)?;
    let cleared = match cleared {
        Some(t) => Some(t),
        None if started.elapsed().as_millis() >= timeout_ms => {
            let now = std::time::Instant::now();
            *cover = Some((started, Some(now)));
            Some(now)
        }
        None => None,
    };
    match cleared {
        None if started.elapsed().as_millis() < grace_ms => None,
        None => Some(1.0_f32),
        // Подтвердилось внутри грейса — шторки не было, гаснуть нечему.
        Some(t) if t.duration_since(started).as_millis() < grace_ms => None,
        Some(t) => {
            let k = t.elapsed().as_millis() as f32 / fade_ms;
            (k < 1.0).then_some(1.0 - k)
        }
    }
}

#[cfg(test)]
mod cover_tests {
    use std::time::{Duration, Instant};

    const TIMEOUT: u128 = 2500;
    const GRACE: u128 = 250;
    const FADE: f32 = 140.0;

    #[test]
    fn форс_снятие_сохраняется_и_шторка_гаснет_а_не_висит_вечно() {
        // Подтверждения готовности не было, таймаут давно прошёл.
        let mut cover = Some((Instant::now() - Duration::from_millis(3000), None));

        // Кадр 1: шторка ещё непрозрачна, но отметка обязана сохраниться.
        let first = super::cover_step(&mut cover, TIMEOUT, GRACE, FADE);
        assert_eq!(first, Some(1.0));
        let stored = cover.expect("состояние не должно исчезать").1;
        assert!(
            stored.is_some(),
            "отметка форс-снятия не сохранена: отсчёт начнётся заново"
        );

        // Кадр 2 после затухания: шторки быть не должно. Прежний код
        // подставлял новый `Instant::now()` и возвращал 1.0 вечно.
        cover = Some((
            Instant::now() - Duration::from_millis(3000),
            Some(Instant::now() - Duration::from_millis(500)),
        ));
        assert_eq!(super::cover_step(&mut cover, TIMEOUT, GRACE, FADE), None);
    }

    #[test]
    fn отметка_ставится_один_раз_и_не_обновляется() {
        // Обновление отметки на каждом кадре и есть исходный дефект.
        let mut cover = Some((Instant::now() - Duration::from_millis(3000), None));
        super::cover_step(&mut cover, TIMEOUT, GRACE, FADE);
        let first = cover.unwrap().1.unwrap();
        std::thread::sleep(Duration::from_millis(30));
        super::cover_step(&mut cover, TIMEOUT, GRACE, FADE);
        assert_eq!(
            cover.unwrap().1.unwrap(),
            first,
            "отметка переписана — затухание не закончится"
        );
    }

    #[test]
    fn в_середине_затухания_прозрачность_между_нулём_и_единицей() {
        let mut cover = Some((
            Instant::now() - Duration::from_millis(3000),
            Some(Instant::now() - Duration::from_millis(70)),
        ));
        let opacity = super::cover_step(&mut cover, TIMEOUT, GRACE, FADE)
            .expect("на середине фейда шторка ещё видна");
        assert!(
            opacity > 0.0 && opacity < 1.0,
            "прозрачность вне диапазона: {opacity}"
        );
    }

    #[test]
    fn подтверждение_внутри_грейса_шторку_не_показывает() {
        let started = Instant::now();
        let mut cover = Some((started, Some(started + Duration::from_millis(100))));
        assert_eq!(super::cover_step(&mut cover, TIMEOUT, GRACE, FADE), None);
    }

    #[test]
    fn внутри_грейса_без_подтверждения_шторки_ещё_нет() {
        let mut cover = Some((Instant::now(), None));
        assert_eq!(super::cover_step(&mut cover, TIMEOUT, GRACE, FADE), None);
        assert!(
            cover.unwrap().1.is_none(),
            "рано ставить отметку: таймаут не вышел"
        );
    }
}
