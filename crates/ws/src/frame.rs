//! Wire-контракт kamin-host (protocol.ts, дословно):
//!   req — {kind:"req", id, method, params[]}
//!   res — {kind:"res", id, ok, value?, error?}
//!   evt — {kind:"evt", channel, payload}

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RpcFrame {
    Req {
        id: u64,
        method: String,
        params: Vec<Value>,
    },
    Res {
        id: u64,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    Evt {
        channel: String,
        payload: Value,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn req_matches_ts_shape() {
        let f = RpcFrame::Req {
            id: 7,
            method: "kamin:fs:listDir".into(),
            params: vec![json!("C:/w")],
        };
        assert_eq!(
            serde_json::to_value(&f).unwrap(),
            json!({"kind":"req","id":7,"method":"kamin:fs:listDir","params":["C:/w"]})
        );
    }

    #[test]
    fn res_ok_and_error_parse() {
        let ok: RpcFrame =
            serde_json::from_str(r#"{"kind":"res","id":1,"ok":true,"value":[1,2]}"#).unwrap();
        assert!(matches!(ok, RpcFrame::Res { ok: true, .. }));
        let err: RpcFrame =
            serde_json::from_str(r#"{"kind":"res","id":2,"ok":false,"error":"boom"}"#).unwrap();
        match err {
            RpcFrame::Res { ok, error, .. } => {
                assert!(!ok);
                assert_eq!(error.as_deref(), Some("boom"));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn evt_parses() {
        let f: RpcFrame =
            serde_json::from_str(r#"{"kind":"evt","channel":"sessions:changed","payload":{}}"#)
                .unwrap();
        assert!(matches!(f, RpcFrame::Evt { .. }));
    }

    #[test]
    fn unknown_fields_do_not_break() {
        // forward-compat (plan/96 §1.1)
        let f: RpcFrame =
            serde_json::from_str(r#"{"kind":"evt","channel":"x","payload":null,"future_field":1}"#)
                .unwrap();
        assert!(matches!(f, RpcFrame::Evt { .. }));
    }
}
