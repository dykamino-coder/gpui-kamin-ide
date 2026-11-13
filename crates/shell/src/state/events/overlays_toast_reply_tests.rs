//! Ответ хосту при снятии тоста любым путём.

use super::pending_shell_reply;

#[test]
fn неотвеченный_запрос_получает_ответ_при_снятии() {
    assert_eq!(pending_shell_reply("shellreq-7", false), Some(7));
}

#[test]
fn уже_отвеченный_запрос_второй_раз_не_отвечается() {
    // Клик по кнопке уже отдал выбранный пункт; ответ `null` поверх него
    // подменил бы выбор пользователя пустотой.
    assert_eq!(pending_shell_reply("shellreq-7", true), None);
}

#[test]
fn тост_без_ожидающего_запроса_ответа_не_требует() {
    assert_eq!(pending_shell_reply("shellmsg-info-привет", false), None);
    assert_eq!(pending_shell_reply("toast-12345", false), None);
}

#[test]
fn нечисловой_идентификатор_запроса_отбрасывается() {
    assert_eq!(pending_shell_reply("shellreq-", false), None);
    assert_eq!(pending_shell_reply("shellreq-abc", false), None);
}
