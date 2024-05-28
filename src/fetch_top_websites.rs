use std::io::{ BufRead, BufReader };
use std::fs::File;
use std::error::Error;
use log::error;

// Fetches the top websites from the file "top-1m.txt"
pub fn fetch_top_websites(num: usize) -> Result<Vec<String>, Box<dyn Error>> {
    let file = File::open("top-1m.txt").map_err(|e| {
        error!("File not found: {}", e);
        e
    })?;
    let reader = BufReader::new(file);
    let mut top_websites = Vec::new();
    for line in reader.lines().take(num) {
        match line {
            Ok(line) => top_websites.push(line),
            Err(e) => {
                error!("Error reading line: {}", e);
                return Err(e.into());
            }
        }
    }
    Ok(top_websites)
}
