//! Решение ватчдога профиля CEF (INC-2026-0073): переход в software обязан
//! забирать ВСЕ браузеры на общих текстурах, а пересоздание — не зацикливаться.

use super::{Plan, Reason, ViewProbe, plan};

fn view(id: &str) -> ViewProbe {
    ViewProbe {
        id: id.into(),
        visible: true,
        accelerated: true,
        has_frame: true,
        opened_secs: Some(30),
        blank: false,
        stall_attempts: 0,
    }
}

fn stalled(id: &str) -> ViewProbe {
    ViewProbe {
        has_frame: false,
        ..view(id)
    }
}

fn ids(plan: &Plan) -> Vec<(&str, Reason)> {
    plan.respawn
        .iter()
        .map(|(id, r)| (id.as_str(), *r))
        .collect()
}

#[test]
fn здоровые_accelerated_вью_не_трогаем() {
    let views = [view("chat"), view("console")];
    assert_eq!(plan(&views, false), Plan::default());
}

#[test]
fn молчащее_вью_переводит_в_software_все_вью_на_общих_текстурах() {
    // Поле владельца: молчали Tools и Agents, а чат и консоль уже получали
    // кадры — их пропуск и давал смешанный режим.
    let hidden_chat = ViewProbe {
        visible: false,
        ..view("chat")
    };
    let views = [
        hidden_chat,
        view("console"),
        stalled("tools"),
        stalled("agents"),
    ];
    let p = plan(&views, false);
    assert!(p.switch_to_software);
    assert_eq!(
        ids(&p),
        vec![
            ("chat", Reason::Migrate),
            ("console", Reason::Migrate),
            ("tools", Reason::Stalled),
            ("agents", Reason::Stalled),
        ]
    );
}

#[test]
fn пустые_accelerated_кадры_тоже_переводят_в_software() {
    let blank = ViewProbe {
        blank: true,
        ..view("chat")
    };
    let p = plan(&[blank, view("console")], false);
    assert!(p.switch_to_software);
    assert_eq!(
        ids(&p),
        vec![("chat", Reason::Blank), ("console", Reason::Migrate)]
    );
}

#[test]
fn в_software_доезжает_поздно_созданный_accelerated_браузер() {
    // Браузер заказан до перехода, а создан после: его профиль — общие
    // текстуры, хотя процесс уже в software.
    let sw = ViewProbe {
        accelerated: false,
        ..view("tools")
    };
    let p = plan(&[sw, view("late")], true);
    assert!(!p.switch_to_software, "переход уже был");
    assert_eq!(ids(&p), vec![("late", Reason::Migrate)]);
}

#[test]
fn после_миграции_пересоздавать_нечего() {
    let sw = |id: &str| ViewProbe {
        accelerated: false,
        ..view(id)
    };
    assert_eq!(plan(&[sw("chat"), sw("console")], true), Plan::default());
}

#[test]
fn простой_ограничен_двумя_попытками() {
    let tired = ViewProbe {
        accelerated: false,
        stall_attempts: 2,
        ..stalled("tools")
    };
    assert_eq!(plan(&[tired], true), Plan::default());
    let second = ViewProbe {
        accelerated: false,
        stall_attempts: 1,
        ..stalled("tools")
    };
    assert_eq!(
        ids(&plan(&[second], true)),
        vec![("tools", Reason::Stalled)]
    );
}

#[test]
fn скрытое_или_молодое_вью_без_кадров_не_простой() {
    let hidden = ViewProbe {
        visible: false,
        ..stalled("hidden")
    };
    let young = ViewProbe {
        opened_secs: Some(11),
        ..stalled("young")
    };
    let starting = ViewProbe {
        opened_secs: None,
        ..stalled("starting")
    };
    assert_eq!(plan(&[hidden, young, starting], false), Plan::default());
}

#[test]
fn пустой_software_кадр_не_повод() {
    // Проба смотрит только accelerated-копию; у software-браузера флаг
    // остаться не может, но решение не должно на него опираться.
    let sw_blank = ViewProbe {
        accelerated: false,
        blank: true,
        ..view("tools")
    };
    assert_eq!(plan(&[sw_blank], true), Plan::default());
}
