use env_logger;

use std::sync::Arc;
use indicatif::MultiProgress;
use tokio::task::JoinHandle;
use tokio::sync::Semaphore;
use std::path::PathBuf;

use num_cpus;

mod database;
mod handle_warc;
mod helper_functions;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Load files to download
    // let files: Vec<String> = helper_functions::fetch_lines(20, "warc.paths").unwrap();

    let files: Vec<String> = database::fetch_files_to_process().await.unwrap();

    println!("Files left: {:?}", files.len());

    // Create a vector to store the tasks
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    let multibar: Arc<MultiProgress> = Arc::new(MultiProgress::new());

    // Limit the number of concurrent tasks to the number of CPU cores
    let num_cpus: usize = num_cpus::get_physical();
    let sem: Arc<Semaphore> = Arc::new(Semaphore::new(num_cpus));

    // let file_path: PathBuf = PathBuf::from("warc_files/test.warc.gz");

    // Read and process the WARC file
    // match handle_warc::read_warc_file(&file_path, &multibar.clone()).await {
    //     Ok(results) => {
    //         match database::add_webpages(&results, &multibar, &file_path).await {
    //             Ok(_) => {}
    //             Err(e) => eprintln!("Error adding webpages to the database: {:?}", e),
    //         }
    //     }
    //     Err(e) => eprintln!("Error reading file: {:?} - {:?}", "test.warc.gz", e),
    // }

    for file in files {
        let permit: Result<
            tokio::sync::OwnedSemaphorePermit,
            tokio::sync::AcquireError
        > = Arc::clone(&sem).acquire_owned().await;

        let file_clone: String = file.clone();
        let multibar: Arc<MultiProgress> = multibar.clone();

        let task: JoinHandle<()> = tokio::spawn(async move {
            let _permit: Result<
                tokio::sync::OwnedSemaphorePermit,
                tokio::sync::AcquireError
            > = permit;

            // Download the WARC file
            let file_path: PathBuf = handle_warc
                ::download_warc_file(file_clone.as_str(), &multibar).await
                .unwrap();
            let mut results: Vec<handle_warc::webpage::Webpage> = Vec::new();
            // Read and process the WARC file
            match handle_warc::read_warc_file(file_path.as_path(), &multibar).await {
                Ok(webpages) => {
                    results = webpages;
                }
                Err(e) => eprintln!("Error reading file: {:?} - {:?}", file_clone, e),
            }

            match database::add_webpages(&results, &multibar, &file_path.as_path()).await {
                Ok(_) => {
                    match database::mark_file_as_processed(&file).await {
                        Ok(_) => {}
                        Err(e) => eprintln!("Error marking file as processed: {:?}", e),
                    }
                }
                Err(e) => eprintln!("Error adding webpages to the database: {:?}", e),
            }

            // Delete the file
            std::fs::remove_file(&file_path).expect("Failed to delete file");
        });
        tasks.push(task);
    }

    // Wait for all tasks to complete.
    for task in tasks {
        task.await.unwrap();
    }
}
