use log::{ info, warn };
use reqwest::Client;
use warc::WarcReader;
use std::error::Error;
use std::path::{ Path, PathBuf };
use url::Url;
use std::time::{ Instant, Duration };
use std::collections::HashSet;
use indicatif::{ MultiProgress, ProgressBar, ProgressStyle };
use std::fs::create_dir_all;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::sync::Arc;
use colored::*;

use crate::helper_functions::fetch_lines;
mod webpage;

fn extract_domain(url: &str) -> Option<String> {
    if let Ok(parsed_url) = Url::parse(url) {
        if let Some(host) = parsed_url.host_str() {
            return Some(host.to_string());
        }
    }
    None
}

pub async fn download_warc_file(
    file_name: &str,
    multibar: Arc<MultiProgress>
) -> Result<PathBuf, Box<dyn Error>> {
    // Create the URL
    let url: String = format!("https://data.commoncrawl.org/{}", file_name);

    // Initialize HTTP client
    let client: Client = Client::new();

    // Create the directory for storing the files
    let dir_path: &Path = Path::new("crawled_data/full_warc_files");
    create_dir_all(&dir_path)?;

    // Extract the filename from the file path
    let path: &Path = Path::new(file_name);
    let file_name: std::borrow::Cow<str> = path.file_name().ok_or("Invalid file name")?.to_string_lossy();

    // Create file paths
    let file_path: PathBuf = dir_path.join(file_name.to_string()).with_extension("gz");

    // Download the file
    let mut response: reqwest::Response = client.get(&url).send().await?.error_for_status()?;
    let mut output_file: File = File::create(&file_path).await?;

    let parts: Vec<&str> = file_name.split("-").collect();
    let file_number: Vec<&str> = parts[parts.len() - 1].split(".").collect();
    let file_number: &str = file_number[0];

    let progress_bar: ProgressBar = multibar.add(
        ProgressBar::new(
            response
                .headers()
                .get("Content-Length")
                .unwrap()
                .to_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
        )
    );
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                &format!(
                    "File {}: [{{elapsed_precise}}] [{{wide_bar:40.cyan/blue}}] {{bytes}}/{{total_bytes}} | Speed: {{bytes_per_sec}} | Time Left: {{eta}}",
                    file_number.to_string().green().bold()
                )
            )
            .unwrap()
            .progress_chars("#>-")
    );

    while let Some(chunk) = response.chunk().await? {
        progress_bar.inc(chunk.len() as u64);
        output_file.write_all(&chunk).await?;
    }
    progress_bar.println(
        format!("{}{}", "Downloaded file to : ".green().bold(), file_path.to_string_lossy().blue())
    );
    progress_bar.finish_and_clear();
    // Return the path to the extracted file
    Ok(file_path)
}

pub async fn read_warc_file(
    file_path: &Path,
    multibar: Arc<MultiProgress>
) -> Result<Vec<webpage::Webpage>, Box<dyn Error>> {
    let top_websites: HashSet<String> = fetch_lines(0, "top-1m.txt")?.into_iter().collect();
    let mut count: i32 = 0;
    let mut matching_count: i32 = 0;
    let mut start: Instant = Instant::now();
    let time_taken: Instant = Instant::now();

    let file_path_string: String = file_path.to_string_lossy().to_string();
    let parts: Vec<&str> = file_path_string.split("-").collect();
    let file_number: Vec<&str> = parts[parts.len() - 1].split(".").collect();
    let file_number: &str = file_number[0];

    let progress_bar = multibar.add(ProgressBar::new(100_000));
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                &format!(
                    "{}: [{{elapsed_precise}}] [{{wide_bar:40.cyan/blue}}] Records read: {{pos}}/≈{{len}} | Time Left: {{eta_precise}} | {{msg}}",
                    format!("File: {}", file_number.to_string().green().bold())
                )
            )
            .unwrap()
            .progress_chars("#>-")
    );
    progress_bar.tick();
    let mut results: Vec<webpage::Webpage> = Vec::new();
    for record in WarcReader::from_path_gzip(file_path)?.iter_records() {
        match record {
            Err(err) => warn!("ERROR: {}", err),
            Ok(record) => {
                let target_uri: String = match record.header(warc::WarcHeader::TargetURI) {
                    Some(uri) => uri.to_string(),
                    None => String::new(),
                };

                match extract_domain(&target_uri) {
                    Some(domain) => {
                        if top_websites.contains(&domain) {
                            if let Ok(webpage) = webpage::Webpage::parse_record(&record) {
                                info!(
                                    "{} | {} | {} | {} | {} | {} | {}",
                                    webpage.warc_date,
                                    webpage.warc_target_uri,
                                    webpage.warc_identified_payload_type,
                                    webpage.status_code,
                                    webpage.content_type,
                                    webpage.content_length,
                                    webpage.text_body.len()
                                );
                                matching_count += 1;
                                results.push(webpage);
                            }
                            let to_increase: u64 = (count as u64) - progress_bar.position();
                            progress_bar.inc(to_increase);
                        }
                    }
                    None => {}
                }
                count += 1;
                if count % 1000 == 0 {
                    let duration: std::time::Duration = start.elapsed();
                    progress_bar.set_message(
                        format!("Time for 1000 records: {:.2} ms", duration.as_secs_f64() * 1000.0)
                            .cyan()
                            .to_string()
                    );
                    // Reset the start time
                    start = Instant::now();
                }
                
            }
        }
    }
    let duration: Duration = time_taken.elapsed();

    let msg: String = format!(
        "{} | {} | {}",
        format!("Finished reading: {}", file_number).green().bold(),
        format!("Time taken overall: {:.2} s", duration.as_secs_f64()).cyan(),
        format!(
            "Matching websites in top {}: {}/{}",
            top_websites.len(),
            matching_count,
            count
        ).yellow()
    );

    progress_bar.println(msg);
    progress_bar.finish_and_clear();

    Ok(results)
}
