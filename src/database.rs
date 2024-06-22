use dotenv::dotenv;
use sqlx::{ PgPool, Row, Postgres, Pool };
use rand::{ thread_rng, Rng };
use std::collections::HashMap;
use std::{ env, path::Path };
use std::error::Error;
use std::time::{ Duration, Instant };
use std::sync::Arc;
use indicatif::{ MultiProgress, ProgressBar, ProgressStyle };
use colored::*;

use crate::handle_warc::webpage::Webpage;
use crate::helper_functions;

const MAX_KEYWORD_LENGTH: usize = 40;

pub async fn add_webpages(
    webpages: &Vec<Webpage>,
    multibar: &Arc<MultiProgress>,
    file_path: &Path
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let file_number: String = helper_functions::file_path_to_number(file_path);
    let progress_bar: ProgressBar = multibar.add(ProgressBar::new(webpages.len() as u64));
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template(
                &format!(
                    "{}: [{{elapsed_precise}}] [{{wide_bar:40.cyan/blue}}] Added to db: {{pos}}/≈{{len}} | Time Left: {{eta}} | {{msg}}",
                    format!("Adding {}", file_number.green().bold())
                )
            )
            .unwrap()
            .progress_chars("#>-")
    );
    progress_bar.tick();

    dotenv().ok();
    let duration: Instant = std::time::Instant::now();
    let database_url: String = env
        ::var("DATABASE_URL")
        .expect("DATABASE_URL must be set in the .env file");
    let pool: Pool<Postgres> = PgPool::connect(&database_url).await.expect(
        "Failed to connect to the database"
    );

    let filtered_webpages: Vec<&Webpage> = webpages
        .iter()
        .filter(|wp| wp.title.is_some() && wp.description.is_some() && wp.warc_target_uri.is_some())
        .collect();

    // Collect all keywords from all webpages
    let mut keyword_counts: HashMap<String, i32> = HashMap::new();
    for wp in &filtered_webpages {
        let keywords: Vec<String> = wp.lemmatised_text.clone().unwrap_or_default();
        let truncated_keywords: Vec<String> = keywords
            .iter()
            .map(|keyword| (
                if keyword.len() > MAX_KEYWORD_LENGTH {
                    keyword[..MAX_KEYWORD_LENGTH].to_owned()
                } else {
                    keyword.clone()
                }
            ))
            .collect();

        for keyword in truncated_keywords {
            *keyword_counts.entry(keyword).or_insert(0) += 1;
        }
    }

    // Batch insert keywords and get their ids
    let mut keyword_id_map: HashMap<String, i32> = HashMap::new();
    if !keyword_counts.is_empty() {
        let insert_keywords_query = format!(
            "INSERT INTO keywords (word, documents_containing_word) VALUES {} ON CONFLICT (word) DO UPDATE SET documents_containing_word = keywords.documents_containing_word + EXCLUDED.documents_containing_word RETURNING id, word",
            keyword_counts
                .keys()
                .enumerate()
                .map(|(i, _)| format!("(${}, 1)", i + 1))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let backoff_delay = Duration::from_millis(thread_rng().gen_range(100..500)); // Random initial delay between 100ms to 500ms
        let mut attempt: i32 = 0;

        loop {
            let mut transaction = pool.begin().await?;
            let mut query = sqlx::query(&insert_keywords_query);

            for keyword in keyword_counts.keys() {
                query = query.bind(keyword);
            }

            let result = query.fetch_all(&mut *transaction).await;

            match result {
                Ok(rows) => {
                    transaction.commit().await?;
                    for row in rows {
                        let keyword_id: i32 = row.get(0);
                        let keyword_word: String = row.get(1);
                        keyword_id_map.insert(keyword_word, keyword_id);
                    }
                    break; // Break out of the retry loop on success
                }
                Err(err) => {
                    transaction.rollback().await?; // Rollback on error
                    if let Some(db_error) = err.as_database_error() {
                        if db_error.code().unwrap_or_default() == "40P01" {
                            attempt += 1;
                            let sleep_duration = backoff_delay * (attempt as u32);
                            tokio::time::sleep(sleep_duration).await;
                            continue;
                        }
                    }
                    return Err(Box::new(err));
                }
            }
        }
    }

    for wp in filtered_webpages.iter() {
        let time_for_webpage: Instant = std::time::Instant::now();
        add_webpage(&wp, &pool, &keyword_id_map).await?;
        let time_taken: f64 = time_for_webpage.elapsed().as_secs_f64();
        let msg: String = format!("Time taken for last webpage: {:.2}s", time_taken)
            .cyan()
            .to_string();
        progress_bar.set_message(msg);
        progress_bar.inc(1);
    }

    let msg: String = format!(
        "{} | {} | {}",
        format!("Added {} to database", file_number).green().bold(),
        format!("Time taken overall: {:.2}s", duration.elapsed().as_secs_f64()).cyan(),
        format!("Number of webpages added: {}", filtered_webpages.len()).yellow()
    );
    progress_bar.println(msg);
    progress_bar.finish_and_clear();
    Ok(())
}

pub async fn add_webpage(
    webpage: &Webpage,
    pool: &PgPool,
    keyword_id_map: &HashMap<String, i32>
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let title: String = webpage.title.clone().unwrap_or_default();
    let description: String = webpage.description.clone().unwrap_or_default();
    let url: String = webpage.warc_target_uri.clone().unwrap_or_default();
    let word_count: i32 = webpage.lemmatised_text.clone().unwrap_or_default().len() as i32;
    let keywords: Vec<String> = webpage.lemmatised_text.clone().unwrap_or_default();
    let links: Vec<String> = webpage.links.clone().unwrap_or_default();

    // Truncate keywords to maximum length
    let truncated_keywords: Vec<String> = keywords
        .iter()
        .map(|keyword| (
            if keyword.len() > MAX_KEYWORD_LENGTH {
                keyword[..MAX_KEYWORD_LENGTH].to_owned()
            } else {
                keyword.clone()
            }
        ))
        .collect();

    // Upsert websites
    let upsert_website_query: &str =
        r#"
    INSERT INTO websites (title, description, url, word_count)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (url) DO UPDATE 
        SET title = EXCLUDED.title, 
            description = EXCLUDED.description, 
            word_count = EXCLUDED.word_count
    RETURNING id, url
    "#;

    let row = sqlx
        ::query(upsert_website_query)
        .bind(&title)
        .bind(&description)
        .bind(&url)
        .bind(&word_count)
        .fetch_one(pool).await?;

    let website_id: i32 = row.get(0);

    // Delete existing keywords and links for this website
    let delete_keywords_query: &str =
        r#"
    DELETE FROM website_keywords WHERE website_id = $1
    "#;
    sqlx::query(delete_keywords_query).bind(&website_id).execute(pool).await?;

    let delete_links_query: &str =
        r#"
    DELETE FROM website_links WHERE source_website_id = $1
    "#;
    sqlx::query(delete_links_query).bind(&website_id).execute(pool).await?;

    // Prepare data for bulk insert of website_keywords
    let mut website_keywords_values: Vec<(i32, i32, i32)> = Vec::new();
    let mut keyword_counts: HashMap<String, i32> = HashMap::new();
    for keyword in truncated_keywords {
        *keyword_counts.entry(keyword).or_insert(0) += 1;
    }

    for (keyword, count) in keyword_counts.iter() {
        if let Some(&keyword_id) = keyword_id_map.get(keyword) {
            website_keywords_values.push((keyword_id, website_id, *count));
        }
    }

    if !website_keywords_values.is_empty() {
        let insert_website_keywords_query = format!(
            "INSERT INTO website_keywords (keyword_id, website_id, keyword_occurrences) VALUES {}",
            website_keywords_values
                .iter()
                .enumerate()
                .map(|(i, _)| format!("(${}, ${}, ${})", i * 3 + 1, i * 3 + 2, i * 3 + 3))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let mut query = sqlx::query(&insert_website_keywords_query);
        for (keyword_id, website_id, keyword_occurrences) in website_keywords_values {
            query = query.bind(keyword_id).bind(website_id).bind(keyword_occurrences);
        }
        query.execute(pool).await?;
    }

    // Prepare data for bulk insert of links
    let links_values: Vec<(i32, &String)> = links
        .iter()
        .map(|link| (website_id, link))
        .collect();

    // Bulk insert links
    if !links_values.is_empty() {
        let insert_links_query = format!(
            "INSERT INTO website_links (source_website_id, target_website) VALUES {} ON CONFLICT (source_website_id, target_website) DO NOTHING",
            links_values
                .iter()
                .enumerate()
                .map(|(i, _)| format!("(${}, ${})", i * 2 + 1, i * 2 + 2))
                .collect::<Vec<_>>()
                .join(", ")
        );

        let mut query = sqlx::query(&insert_links_query);
        for (source_website_id, target_website) in links_values {
            query = query.bind(source_website_id).bind(target_website);
        }
        query.execute(pool).await?;
    }

    Ok(())
}
