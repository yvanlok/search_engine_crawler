use serde_json::Value;
use log::{ info, error };
use env_logger;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;

mod fetch_page;
mod fetch_top_websites;

#[tokio::main]
async fn main() {
    // Initialize the logger
    env_logger::init();

    // Fetch top websites
    let mut websites: Vec<String> = Vec::new();
    match fetch_top_websites::fetch_top_websites(1000) {
        Ok(top_websites) => {
            for website in top_websites {
                websites.push(website);
            }
        }
        Err(e) => {
            error!("Error fetching top websites: {}", e);
            return;
        }
    }

    // Initialize the progress bar
    let mut successful = 0;
    let mut failed = 0;
    let progress_bar = ProgressBar::new(websites.len() as u64);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("{prefix} | [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} | {msg}")
            .unwrap()
            .progress_chars("#>-")
    );

    for website in websites {
        match fetch_page::fetch_index(&website).await {
            Ok(results) => {
                // Filter results that match the criteria: status is 200 and mime-detected is "text/html" and languages contain "eng"
                let filtered_results: Vec<&serde_json::Value> = results
                    .iter()
                    .filter(
                        |result|
                            convert_to_u64(&result["status"]) == 200 &&
                            result["mime-detected"].as_str().unwrap_or("") == "text/html" &&
                            result["languages"].as_str().unwrap_or("").contains("eng")
                    )
                    .collect();

                // Check if there are any filtered results
                if filtered_results.is_empty() {
                    info!("No successful crawl found for {}", website);
                } else {
                    // Find the latest crawl by comparing timestamps
                    let latest_crawl = filtered_results
                        .iter()
                        .max_by_key(|result| convert_to_u64(&result["timestamp"]))
                        .unwrap(); // Safe to unwrap because we checked for empty

                    // Extract offset and length
                    let offset = convert_to_u64(&latest_crawl["offset"]);
                    let length = convert_to_u64(&latest_crawl["length"]);

                    // Extract URL
                    let url_parts: Vec<&str> = latest_crawl["urlkey"]
                        .as_str()
                        .unwrap()
                        .trim_end_matches(")/") // Remove )/ from the end
                        .split(",")
                        .collect();
                    let url_parts: Vec<&str> = url_parts.into_iter().rev().collect();
                    let url = url_parts.join(".");

                    // Download the page with extracted options
                    match
                        fetch_page::download_page(
                            url.as_str(),
                            latest_crawl["filename"].as_str().unwrap(),
                            offset as usize,
                            length as usize
                        ).await
                    {
                        Ok(file_path) => {
                            info!("Downloaded text file to: {}", file_path.display());
                            successful += 1;
                            progress_bar.set_message(
                                format!("\x1B[32mSuccessfully downloaded page for {}\x1B[0m", website)
                            );
                        }
                        Err(e) => {
                            // Update progress bar with error message
                            failed += 1;
                            progress_bar.set_message(
                                format!(
                                    "\x1B[31mError downloading page for {}: {}\x1B[0m",
                                    website,
                                    e
                                )
                            );
                        }
                    }
                }
            }
            Err(e) => {
                // Update progress bar with error message
                failed += 1;
                progress_bar.set_message(
                    format!("\x1B[31mError fetching index for {}: {}\x1B[0m", website, e)
                );
            }
        }
        // Update the progress bar
        progress_bar.set_prefix(
            format!("\x1B[32m{}\x1B[0m successful, \x1B[31m{}\x1B[0m failed", successful, failed)
        );
        progress_bar.inc(1);
    }
    progress_bar.finish_with_message("Completed!");
}

// Helper function to convert serde Value to u64
fn convert_to_u64(value: &Value) -> u64 {
    value.as_str().unwrap().parse::<u64>().unwrap()
}
