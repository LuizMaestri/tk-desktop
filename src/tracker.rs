use std::collections::HashMap;

use crate::estimate::estimate_tokens;
use crate::frames::Frame;

/// Evento de medição: uma requisição casada com sua resposta.
/// Só tamanhos/contagens — nunca conteúdo de payload (privacidade).
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub ts: chrono::DateTime<chrono::Utc>,
    pub server: String,
    pub tool: String,
    pub req_tokens: u64,
    pub resp_tokens: u64,
}

/// Casa requisições com respostas pelo id JSON-RPC
/// (a resposta não carrega o método, só o id).
pub struct Tracker {
    server: String,
    pending: HashMap<String, (String, usize)>, // id -> (label, params_len)
}

impl Tracker {
    pub fn new(server: String) -> Self {
        Tracker { server, pending: HashMap::new() }
    }

    /// Alimenta um frame; devolve um Event completo quando uma resposta casa.
    pub fn observe(&mut self, frame: Frame) -> Option<Event> {
        match frame {
            Frame::Request { id, label, params_len } => {
                self.pending.insert(id, (label, params_len));
                None
            }
            Frame::Response { id, payload_len } => {
                let (label, params_len) = self.pending.remove(&id)?;
                Some(Event {
                    ts: chrono::Utc::now(),
                    server: self.server.clone(),
                    tool: label,
                    req_tokens: estimate_tokens(params_len),
                    resp_tokens: estimate_tokens(payload_len),
                })
            }
            Frame::Passthrough => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::Frame;

    fn req(id: &str, label: &str, params_len: usize) -> Frame {
        Frame::Request { id: id.into(), label: label.into(), params_len }
    }

    #[test]
    fn matches_response_to_request_by_id() {
        let mut t = Tracker::new("srv".into());
        assert!(t.observe(req("1", "echo", 40)).is_none());
        let e = t.observe(Frame::Response { id: "1".into(), payload_len: 400 }).unwrap();
        assert_eq!(e.server, "srv");
        assert_eq!(e.tool, "echo");
        assert_eq!(e.req_tokens, 10);
        assert_eq!(e.resp_tokens, 100);
    }

    #[test]
    fn unmatched_response_yields_nothing() {
        let mut t = Tracker::new("srv".into());
        assert!(t.observe(Frame::Response { id: "9".into(), payload_len: 4 }).is_none());
    }

    #[test]
    fn interleaved_ids_match_correctly() {
        let mut t = Tracker::new("srv".into());
        t.observe(req("1", "a", 4));
        t.observe(req("2", "b", 8));
        assert_eq!(t.observe(Frame::Response { id: "2".into(), payload_len: 4 }).unwrap().tool, "b");
        assert_eq!(t.observe(Frame::Response { id: "1".into(), payload_len: 4 }).unwrap().tool, "a");
    }

    #[test]
    fn passthrough_yields_nothing() {
        let mut t = Tracker::new("srv".into());
        assert!(t.observe(Frame::Passthrough).is_none());
    }
}
