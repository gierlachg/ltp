use crate::Command::{self, ERRONEOUS, GET, QUIT, SHUTDOWN};

pub(super) trait Protocol<T> {
    fn decode(&self, input: T) -> Command;

    fn encode(&self, line: Option<String>) -> T;
}

/// An implementation of string/line based client-server protocol.
#[derive(Clone)]
pub(super) struct StringProtocol;

impl StringProtocol {
    pub(super) fn new() -> Self {
        StringProtocol
    }
}

impl Protocol<String> for StringProtocol {
    fn decode(&self, s: String) -> Command {
        match s.as_ref() {
            "QUIT" => QUIT,
            "SHUTDOWN" => SHUTDOWN,
            line if line.starts_with("GET ") => match line.split_once(" ") {
                Some((_, line_number)) => match line_number.parse::<i64>() {
                    Ok(line_number) if line_number > 0 => GET(line_number as u64),
                    _ => ERRONEOUS,
                },
                None => ERRONEOUS,
            },
            _ => ERRONEOUS,
        }
    }

    fn encode(&self, line: Option<String>) -> String {
        line.map_or("ERR\r\n".to_string(), |mut line| {
            line.insert_str(0, "OK\r\n");
            line
        })
    }
}
