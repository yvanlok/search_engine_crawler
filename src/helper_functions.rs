use std::io::{ BufRead, BufReader };
use std::fs::File;
use std::error::Error;
use std::collections::HashSet;
use std::path::Path;
use lazy_static::lazy_static;
use url::Url;
// use rust_lemmatizer::get_words_from_string;
use std::collections::HashMap;
use once_cell::sync::Lazy;
use regex::Regex;

use markup5ever_rcdom::{ Handle, NodeData, RcDom };

pub fn extract_domain_from_string(url: &str) -> Option<String> {
    if let Ok(parsed_url) = Url::parse(url) {
        if let Some(host) = parsed_url.host_str() {
            return Some(host.to_string());
        }
    }
    None
}

// Function to fetch lines from a file.
pub fn fetch_lines(num: usize, file_path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    // Open the file
    let file: File = File::open(file_path)?;

    // Create a buffered reader
    let reader: BufReader<File> = BufReader::new(file);

    // Read the lines from the file
    let lines: Vec<String> = if num == 0 {
        // Read all lines if num is 0
        reader.lines().collect::<Result<Vec<String>, _>>()?
    } else {
        // Read only the specified number of lines
        reader.lines().take(num).collect::<Result<Vec<String>, _>>()?
    };

    Ok(lines)
}

// Global static for storing the lemma mappings.
static LEMMA_MAP: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut map = HashMap::new();
    let lines = fetch_lines(0, "lemmatised_words.txt").expect("Failed to read lemma file");

    let re = Regex::new(r"^([^/]+)[^->]*->(.+)$").unwrap();
    for entry in lines {
        if let Some(captures) = re.captures(&entry) {
            let lemma = captures[1].trim().to_string();
            let words: Vec<&str> = captures[2]
                .split(',')
                .map(|word| word.trim())
                .collect();
            for word in words {
                map.insert(word.to_string(), lemma.clone());
            }
        }
    }
    map
});

// Function to lemmatize a given string.
pub fn lemmatise_string(text: &str) -> Vec<String> {
    let re: Regex = Regex::new(r"[^a-zA-Z0-9\s]").unwrap(); // regex to match non-word characters
    let text_no_punct = re.replace_all(text, ""); // remove punctuation
    text_no_punct
        .split_whitespace()
        .map(|word: &str| {
            LEMMA_MAP.get(word.to_lowercase().as_str()).unwrap_or(&word.to_string()).clone()
        })
        .collect()
}

// Optimized function signature to pass tags set as parameter
pub fn extract_text_from_html(dom: &RcDom) -> Result<String, Box<dyn Error>> {
    let mut visible_text: String = String::new();
    extract_visible_text(&dom.document, &mut visible_text, &TAGS_WITH_LINE_BREAK);

    Ok(visible_text.trim().to_string())
}

// Optimized recursive function with borrowed result and tags set
fn extract_visible_text(
    handle: &Handle,
    result: &mut String,
    tags_with_line_break: &HashSet<&'static str>
) {
    let node = handle;
    match node.data {
        NodeData::Document => {
            for child in node.children.borrow().iter() {
                extract_visible_text(child, result, tags_with_line_break);
            }
        }
        NodeData::Element { ref name, .. } => {
            let tag_name: &_ = name.local.as_ref();
            if tag_name != "script" && tag_name != "style" {
                for child in node.children.borrow().iter() {
                    extract_visible_text(child, result, tags_with_line_break);
                }
                if tags_with_line_break.contains(tag_name) {
                    result.push('\n');
                }
            }
        }
        NodeData::Text { ref contents } => {
            let text = &contents.borrow();
            if !text.is_empty() {
                result.push_str(text);
            }
        }
        _ => {}
    }
}

// Reusing the HashSet by passing it as parameter
lazy_static! {
    static ref TAGS_WITH_LINE_BREAK: HashSet<&'static str> = {
        let mut set = HashSet::new();
        set.insert("br");
        set.insert("p");
        set.insert("div");
        set.insert("li");
        set
    };
}

pub fn extract_links_from_html(dom: &RcDom, base_url: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut links: Vec<String> = Vec::new();
    let base: Url = Url::parse(base_url)?;

    extract_links(&dom.document, &mut links, &base);
    Ok(links)
}

fn extract_links(handle: &Handle, links: &mut Vec<String>, base: &Url) {
    let node = handle;
    match node.data {
        NodeData::Document => {
            for child in node.children.borrow().iter() {
                extract_links(child, links, base);
            }
        }
        NodeData::Element { ref name, ref attrs, .. } => {
            let tag_name = name.local.as_ref();
            if tag_name == "a" {
                for attr in attrs.borrow().iter() {
                    if attr.name.local.as_ref() == "href" {
                        let href: String = attr.value.to_string();
                        match base.join(&href) {
                            Ok(full_url) => links.push(full_url.to_string()),
                            Err(_) => links.push(href),
                        }
                    }
                }
            }
            for child in node.children.borrow().iter() {
                extract_links(child, links, base);
            }
        }
        _ => {}
    }
}

pub fn extract_title_from_html(node: &Handle) -> Option<String> {
    match node.data {
        NodeData::Element { ref name, .. } => {
            let tag_name = name.local.as_ref();
            if tag_name == "title" {
                for child in node.children.borrow().iter() {
                    if let NodeData::Text { ref contents } = child.data {
                        return Some(contents.borrow().to_string());
                    }
                }
            }
        }
        _ => {}
    }
    for child in node.children.borrow().iter() {
        if let Some(title) = extract_title_from_html(child) {
            return Some(title);
        }
    }
    None
}

pub fn extract_description_from_html(node: &Handle) -> Option<String> {
    match node.data {
        NodeData::Element { ref name, .. } => {
            let tag_name = name.local.as_ref();
            if tag_name == "description" {
                for child in node.children.borrow().iter() {
                    if let NodeData::Text { ref contents } = child.data {
                        return Some(contents.borrow().to_string());
                    }
                }
            }
        }
        _ => {}
    }
    for child in node.children.borrow().iter() {
        if let Some(title) = extract_title_from_html(child) {
            return Some(title);
        }
    }
    None
}

pub fn file_path_to_number(file_path: &Path) -> String {
    let file_path_string: String = file_path.to_string_lossy().to_string();
    let parts: Vec<&str> = file_path_string.split("-").collect();
    let file_number: Vec<&str> = parts[parts.len() - 1].split(".").collect();
    let file_number: &str = file_number[0];

    file_number.to_string()
}
