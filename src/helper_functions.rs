use std::io::{ BufRead, BufReader };
use std::fs::File;
use std::error::Error;
use log::error;
use std::collections::HashSet;
use lazy_static::lazy_static;
use url::Url;

use markup5ever_rcdom::{ Handle, NodeData, RcDom };

pub fn extract_domain_from_string(url: &str) -> Option<String> {
    if let Ok(parsed_url) = Url::parse(url) {
        if let Some(host) = parsed_url.host_str() {
            return Some(host.to_string());
        }
    }
    None
}

pub fn fetch_lines(num: usize, file_path: &str) -> Result<Vec<String>, Box<dyn Error>> {
    // Open the file
    let file: File = File::open(file_path).map_err(|e| {
        error!("File not found: {}", e);
        e
    })?;

    // Create a buffered reader
    let reader: BufReader<File> = BufReader::new(file);

    // Initialize the vector to store the lines
    let mut top_websites: Vec<String> = Vec::new();

    // Read the lines from the file
    let lines: Vec<String> = if num == 0 {
        // Read all lines if num is 0
        reader.lines().collect::<Result<Vec<String>, _>>()?
    } else {
        // Read only the specified number of lines
        reader.lines().take(num).collect::<Result<Vec<String>, _>>()?
    };

    // Add each line to the vector
    for line in lines {
        top_websites.push(line);
    }

    // Return the vector of lines
    Ok(top_websites)
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
