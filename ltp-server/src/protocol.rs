use crate::Request::{self, ERRONEOUS, GET, QUIT, SHUTDOWN};
use crate::Response::{self, ERROR, LINE};

pub(super) trait Protocol<T> {
    fn decode(&self, request: T) -> Request;

    fn encode(&self, response: Response) -> T;
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
    fn decode(&self, request: String) -> Request {
        match request.as_ref() {
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

    fn encode(&self, response: Response) -> String {
        match response {
            LINE(mut line) => {
                line.insert_str(0, "OK\r\n");
                line
            }
            ERROR => "ERR\r\n".to_string(),
        }
    }
}
