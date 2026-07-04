use serde_json::Value;

/// O que uma linha do transporte stdio MCP representa para a medição.
#[derive(Debug, PartialEq)]
pub enum Frame {
    /// Requisição com id: rótulo (tool ou método) + tamanho dos params serializados.
    Request {
        id: String,
        label: String,
        params_len: usize,
    },
    /// Resposta (result ou error) com id + tamanho do payload serializado.
    Response { id: String, payload_len: usize },
    /// Notificação, linha não-JSON ou formato não reconhecido: só repassar.
    Passthrough,
}

/// Inspeciona uma linha sem modificá-la. Nunca falha: o que não for
/// reconhecido vira Passthrough (a transparência é a garantia nº 1).
pub fn inspect(line: &[u8]) -> Frame {
    let value: Value = match serde_json::from_slice(line) {
        Ok(v) => v,
        Err(_) => return Frame::Passthrough,
    };
    let id = match value.get("id") {
        Some(id) if !id.is_null() => id.to_string(),
        _ => return Frame::Passthrough,
    };
    if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
        let params = value.get("params");
        let label = if method == "tools/call" {
            params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or(method)
                .to_string()
        } else {
            method.to_string()
        };
        let params_len = params.map(|p| p.to_string().len()).unwrap_or(0);
        Frame::Request {
            id,
            label,
            params_len,
        }
    } else if let Some(payload) = value.get("result").or_else(|| value.get("error")) {
        Frame::Response {
            id,
            payload_len: payload.to_string().len(),
        }
    } else {
        Frame::Passthrough
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_call_request_uses_tool_name_as_label() {
        let line = br#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"echo","arguments":{"msg":"oi"}}}"#;
        let Frame::Request {
            id,
            label,
            params_len,
        } = inspect(line)
        else {
            panic!("esperava Request");
        };
        assert_eq!(id, "1");
        assert_eq!(label, "echo");
        assert_eq!(
            params_len,
            r#"{"name":"echo","arguments":{"msg":"oi"}}"#.len()
        );
    }

    #[test]
    fn other_request_uses_method_as_label() {
        let line = br#"{"jsonrpc":"2.0","id":"a","method":"tools/list"}"#;
        let Frame::Request {
            id,
            label,
            params_len,
        } = inspect(line)
        else {
            panic!("esperava Request");
        };
        assert_eq!(id, "\"a\"");
        assert_eq!(label, "tools/list");
        assert_eq!(params_len, 0);
    }

    #[test]
    fn response_measures_result_payload() {
        let line = br#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
        assert_eq!(
            inspect(line),
            Frame::Response {
                id: "1".into(),
                payload_len: r#"{"ok":true}"#.len()
            }
        );
    }

    #[test]
    fn error_response_measures_error_payload() {
        let line = br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"x"}}"#;
        let Frame::Response { payload_len, .. } = inspect(line) else {
            panic!("esperava Response");
        };
        assert!(payload_len > 0);
    }

    #[test]
    fn notification_and_garbage_are_passthrough() {
        assert_eq!(
            inspect(br#"{"jsonrpc":"2.0","method":"notifications/progress"}"#),
            Frame::Passthrough
        );
        assert_eq!(inspect(b"not json\n"), Frame::Passthrough);
        assert_eq!(inspect(b""), Frame::Passthrough);
    }

    #[test]
    fn trailing_newline_is_tolerated() {
        let line = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
        assert!(matches!(inspect(line), Frame::Request { .. }));
    }
}
