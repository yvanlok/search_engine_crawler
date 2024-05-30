use std::io::{ BufRead, BufReader };
use std::fs::File;
use std::error::Error;
use log::error;

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
