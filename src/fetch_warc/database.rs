use std::error::Error;

use crate::fetch_warc::Webpage;

pub async fn add_webpage(page: &Webpage) -> Result<(), Box<dyn Error>> {
    Ok(())
}
