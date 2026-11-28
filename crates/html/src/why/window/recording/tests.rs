//! Request completion follows the observed presentation.

use super::*;

#[test]
fn an_old_or_unpresented_frame_cannot_complete_a_new_request() {
    let mut frame = json!({"request_id": 1, "presented": true});
    assert!(requested_presentation(&frame, 1));
    assert!(!requested_presentation(&frame, 2));
    frame["request_id"] = json!(2);
    frame["presented"] = json!(false);
    assert!(!requested_presentation(&frame, 2));
    frame["presented"] = json!(true);
    assert!(requested_presentation(&frame, 2));
    assert!(!requested_presentation(&json!({"presented": true}), 0));
}
