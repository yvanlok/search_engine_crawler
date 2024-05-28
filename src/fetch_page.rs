use flate2::read::GzDecoder;
use reqwest;
use std::fs::{ File, remove_file, create_dir_all };
use std::io::{ copy, Cursor, Read, Write, BufRead, BufReader };
use std::path::{ Path, PathBuf };
use std::error::Error;
use html2text::from_read;
use serde_json::{ Value, from_str };
use log::error;

// Fetches the index of the specified website from Common Crawl
pub async fn fetch_index(website: &str) -> Result<Vec<Value>, Box<dyn Error>> {
    let latest_index = "CC-MAIN-2024-18-index";
    let url = format!("https://index.commoncrawl.org/{}?url={}&output=json", latest_index, website);

    let client = reqwest::Client::new();
    let response = client.get(&url).send().await?;

    let mut results = Vec::new();

    match response.status() {
        reqwest::StatusCode::OK => {
            let body = response.text().await?;
            for line in body.lines() {
                let parsed: Value = from_str(line)?;
                results.push(parsed);
            }
            Ok(results)
        }
        reqwest::StatusCode::NOT_FOUND => Err("No crawls found".into()),
        other => Err(format!("Unexpected status code: {}", other).into()),
    }
}

// Downloads a page and processes it to extract text content
pub async fn download_page(
    website: &str,
    warc_url: &str,
    offset: usize,
    length: usize
) -> Result<PathBuf, Box<dyn Error>> {
    match download_page_as_warc(website, warc_url, offset, length).await {
        Ok(file_path) => {
            match get_html(&file_path.to_string_lossy()) {
                Ok(html_file_path) => {
                    match get_text(&html_file_path.to_string_lossy()) {
                        Ok(text_file_path) => Ok(text_file_path),
                        Err(e) => {
                            error!("Error extracting text: {}", e);
                            Err(e)
                        }
                    }
                }
                Err(e) => {
                    error!("Error extracting HTML: {}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            error!("Error downloading WARC file: {}", e);
            Err(e)
        }
    }
}

// Downloads the page as a WARC file and unzips it
async fn download_page_as_warc(
    website: &str,
    file_name: &str,
    offset: usize,
    length: usize
) -> Result<PathBuf, Box<dyn Error>> {
    let url = format!("https://data.commoncrawl.org/{}", file_name);
    let client = reqwest::Client::new();
    let offset_end = offset + length - 1;

    let response = client
        .get(&url)
        .header(reqwest::header::RANGE, format!("bytes={}-{}", offset, offset_end))
        .send().await?;

    create_dir_all("crawled_data/warc_files")?;

    let file_path = Path::new("crawled_data/warc_files").join(format!("{}.warc.gz", website));
    let mut dest = File::create(&file_path)?;
    let mut content = Cursor::new(response.bytes().await?);
    copy(&mut content, &mut dest)?;

    // Unzip the file
    let mut gz = GzDecoder::new(File::open(&file_path)?);
    let mut s = Vec::new();
    gz.read_to_end(&mut s)?;

    let unzipped_path = Path::new("crawled_data/warc_files").join(format!("{}.warc", website));
    let mut unzipped_file = File::create(&unzipped_path)?;
    unzipped_file.write_all(&s)?;

    // Delete the compressed file
    remove_file(file_path)?;

    Ok(unzipped_path)
}

// Extracts HTML content from the unzipped WARC file
fn get_html(file_path: &str) -> Result<PathBuf, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut html = String::new();
    let reader = BufReader::new(file);

    let mut is_http_header = false;
    let mut is_html_content = false;
    let mut first_html_tag_found = false;

    for line in reader.lines() {
        let line = line?;
        if line.starts_with("WARC/") {
            is_http_header = false;
            is_html_content = false;
        } else if line.starts_with("Content-Type: application/http;") {
            is_http_header = true;
        } else if is_http_header && line.starts_with("HTTP/") {
            is_http_header = false;
        } else if !first_html_tag_found && line.trim_start().contains("<html") {
            html.push_str(&line);
            html.push('\n');
            is_html_content = true;
            first_html_tag_found = true;
        } else if is_html_content {
            html.push_str(&line);
            html.push('\n');
        }
    }

    create_dir_all("crawled_data/html_files")?;
    let html_file_path = Path::new("crawled_data/html_files").join(
        format!("{}.html", Path::new(file_path).file_stem().unwrap().to_string_lossy())
    );
    let mut html_file = File::create(&html_file_path)?;
    html_file.write_all(html.as_bytes())?;

    Ok(html_file_path)
}

// Extracts plain text from the HTML content
pub fn get_text(file_path: &str) -> Result<PathBuf, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut lines = Vec::new();
    for line in reader.lines() {
        let line = line?;
        lines.push(line);
    }
    let text = from_read(lines.join("\n").as_bytes(), 80);

    create_dir_all("crawled_data/text_files")?;
    let text_file_path = Path::new("crawled_data/text_files").join(
        format!("{}.txt", Path::new(file_path).file_stem().unwrap().to_string_lossy())
    );
    let mut text_file = File::create(&text_file_path)?;
    text_file.write_all(text.as_bytes())?;

    Ok(text_file_path)
}
