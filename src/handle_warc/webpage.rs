use warc::{ WarcHeader, Record, BufferedBody };
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::RcDom;
use whichlang::{ detect_language, Lang };
use std::str::from_utf8;

use crate::helper_functions;

#[derive(Debug, Clone)]
pub struct Webpage {
    pub warc_date: Option<String>,
    pub warc_target_uri: Option<String>,
    pub warc_identified_payload_type: Option<String>,
    pub status_code: Option<usize>,
    pub content_type: Option<String>,
    pub content_length: Option<usize>,
    pub html_body: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub links: Option<Vec<String>>,
    pub text_body: Option<String>,
    pub lang: Option<Lang>,
    pub lemmatised_text: Option<Vec<String>>,
}

impl Webpage {
    pub fn parse_record(record: &Record<BufferedBody>) -> Result<Option<Self>, ParseError> {
        // Convert body bytes to UTF-8 string
        let body: &str = from_utf8(record.body()).map_err(|e|
            ParseError::BodyEncodingError(e.to_string())
        )?;

        let mut content_type: Option<String> = None;
        let mut status_code: Option<usize> = None;
        let mut html_body: String = String::new();
        let mut header_processed: bool = false;

        for line in body.lines() {
            if line.starts_with("Content-Type") {
                content_type = extract_content_type(line);
                header_processed = true;
                if
                    content_type.as_ref().is_none() ||
                    !content_type.as_ref().unwrap().contains("text/html")
                {
                    return Ok(None);
                }
            } else if line.starts_with("HTTP") {
                status_code = extract_status(line);
                header_processed = true;
            } else if header_processed {
                if line.contains("<html") || !html_body.is_empty() {
                    html_body.push_str(line);
                }
            }
        }

        match content_type.as_ref() {
            Some(content_type) if content_type.contains("text/html") && !html_body.is_empty() => {
                let parser: html5ever::Parser<RcDom> = parse_document(
                    RcDom::default(),
                    Default::default()
                );
                let dom: RcDom = parser.one(html_body.clone());

                let warc_date: Option<String> = record.header(WarcHeader::Date).map_or_else(
                    || None,
                    |date| Some(date.to_string())
                );
                let warc_target_uri: Option<String> = record
                    .header(WarcHeader::TargetURI)
                    .map_or_else(
                        || None,
                        |uri| Some(uri.to_string())
                    );
                let warc_identified_payload_type: Option<String> = record
                    .header(WarcHeader::IdentifiedPayloadType)
                    .map_or_else(
                        || None,
                        |payload_type| Some(payload_type.to_string())
                    );

                let content_length: Option<usize> = record
                    .header(WarcHeader::ContentLength)
                    .map_or_else(
                        || None,
                        |content_length| Some(content_length.parse().unwrap())
                    );
                let mut lang: Option<Lang> = None;
                let text_body: Option<String> = {
                    let temp: String = extract_text_body(&dom).unwrap_or_default();
                    if temp.is_empty() {
                        None
                    } else {
                        lang = Some(detect_language(temp.as_str()));
                        Some(temp)
                    }
                };

                if lang.is_none() || lang.unwrap() != Lang::Eng {
                    return Ok(None);
                }

                let lemmatised_text: Option<Vec<String>> = match text_body.as_ref() {
                    Some(text) => Some(helper_functions::lemmatise_string(&text.to_lowercase())),
                    None => None,
                };

                // Generate the Webpage struct from the parsed data
                let result: Webpage = Webpage {
                    warc_date,
                    warc_target_uri: warc_target_uri.clone(),
                    warc_identified_payload_type,
                    status_code,
                    content_type: Some(content_type.to_string()),
                    content_length,
                    html_body: Some(html_body),
                    title: extract_title(&dom),
                    description: extract_description(&dom),
                    links: {
                        let links: Vec<String> = helper_functions
                            ::extract_links_from_html(&dom, &warc_target_uri.unwrap_or_default())
                            .unwrap_or_default();
                        if links.is_empty() {
                            None
                        } else {
                            Some(links)
                        }
                    },
                    text_body,
                    lang,
                    lemmatised_text,
                };
                Ok(Some(result))
            }
            _ => Ok(None),
        }
    }
}

// Extract the textual content from HTML body, capturing potential errors
fn extract_text_body(dom: &RcDom) -> Result<String, Box<dyn std::error::Error>> {
    match helper_functions::extract_text_from_html(dom) {
        Ok(text) => Ok(text),
        Err(e) => Err(e),
    }
}

// Extract content type from a header line
fn extract_content_type(line: &str) -> Option<String> {
    line.split(':')
        .nth(1)
        .map(|s| s.trim().to_string())
}

// Extract status code from a status line
fn extract_status(line: &str) -> Option<usize> {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
}

// Extract webpage title from HTML
fn extract_title(dom: &RcDom) -> Option<String> {
    helper_functions::extract_title_from_html(&dom.document)
}

// Extract webpage title from HTML
fn extract_description(dom: &RcDom) -> Option<String> {
    helper_functions::extract_description_from_html(&dom.document)
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
