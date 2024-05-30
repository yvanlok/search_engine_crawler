use warc::{ WarcHeader, Record, BufferedBody };
use html2text::from_read;
use std::str::from_utf8;
use std::panic::{ self };

#[derive(Debug)]
pub struct Webpage {
    pub warc_date: String,
    pub warc_target_uri: String,
    pub warc_identified_payload_type: String,
    pub status_code: usize,
    pub content_type: String,
    pub content_length: usize,
    pub html_body: String,
    pub text_body: String,
}

impl Webpage {
    pub fn parse_record(record: &Record<BufferedBody>) -> Result<Self, ParseError> {
        // Convert body bytes to UTF-8 string
        let body = from_utf8(record.body()).map_err(|e|
            ParseError::BodyEncodingError(e.to_string())
        )?;

        let mut content_type = None;
        let mut status_code = None;
        let mut html_body = String::new();

        for line in body.lines() {
            if line.starts_with("Content-Type") {
                content_type = extract_content_type(line);
            } else if line.starts_with("HTTP") {
                status_code = extract_status(line);
            } else if line.contains("<html") {
                html_body.push_str(line);
            }
            // Break early if both fields are found
            if content_type.is_some() && status_code.is_some() {
                break;
            }
        }

        // Generate the Webpage struct from the parsed data
        Ok(Webpage {
            warc_date: record.header(WarcHeader::Date).map_or_else(
                || "No Date found".to_string(),
                |date| date.to_string()
            ),
            warc_target_uri: record.header(WarcHeader::TargetURI).map_or_else(
                || "No TargetURI found".to_string(),
                |uri| uri.to_string()
            ),
            warc_identified_payload_type: record
                .header(WarcHeader::IdentifiedPayloadType)
                .map_or_else(
                    || "No IdentifiedPayloadType found".to_string(),
                    |payload_type| payload_type.to_string()
                ),
            status_code: status_code.unwrap_or(0),
            content_type: content_type.unwrap_or_default(),
            content_length: record
                .header(WarcHeader::ContentLength)
                .map_or(0, |length| length.parse().unwrap_or(0)),
            html_body: html_body.clone(),
            text_body: extract_text_body(&html_body).unwrap_or_default(),
        })
    }
}

// Extract the textual content from HTML body, capturing potential errors
fn extract_text_body(html_body: &str) -> Result<String, Box<dyn std::any::Any + Send>> {
    let result = panic::catch_unwind(|| from_read(html_body.as_bytes(), 80));
    result.map_err(|err| {
        eprintln!("Error while extracting text body: {:?}", err);
        err
    })
}

// Extract content type from a header line
fn extract_content_type(line: &str) -> Option<String> {
    line.split_whitespace()
        .nth(1)
        .map(|s| s.to_string())
}

// Extract status code from a status line
fn extract_status(line: &str) -> Option<usize> {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
}

#[derive(Debug)]
pub enum ParseError {
    BodyEncodingError(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::BodyEncodingError(msg) => write!(f, "Body encoding error: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}
