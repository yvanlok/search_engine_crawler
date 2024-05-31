use env_logger;
use std::sync::Arc;
use indicatif::MultiProgress;
use tokio::task::JoinHandle;
use tokio::sync::Semaphore;
use std::path::PathBuf;
use num_cpus;

mod fetch_warc;
mod helper_functions;

#[tokio::main]
async fn main() {
    env_logger::init();
    // Load files to download
    let files: Vec<String> = helper_functions::fetch_lines(20, "warc.paths").unwrap();

    // Create a vector to store the tasks
    let mut tasks: Vec<JoinHandle<()>> = Vec::new();

    let multibar: Arc<MultiProgress> = Arc::new(MultiProgress::new());

    // Limit the number of concurrent tasks to the number of CPUs
    let num_cpus: usize = num_cpus::get();
    let sem: Arc<Semaphore> = Arc::new(Semaphore::new(num_cpus));

    // Read and process the WARC file
    // match
    //     fetch_warc::read_warc_file(
    //         PathBuf::from("crawled_data/full_warc_files/test.warc.gz").as_path(),
    //         &multibar
    //     ).await
    // {
    //     Ok(results) => {
    //         for result in results {
    //             if result.text_body.is_some() {
    //                 // println!("First valid result: {:?}", result);
    //                 // break;
    //             }
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
            let file_path: PathBuf = fetch_warc
                ::download_warc_file(file_clone.as_str(), &multibar).await
                .unwrap();

            // Read and process the WARC file
            match fetch_warc::read_warc_file(file_path.as_path(), &multibar).await {
                Ok(results) => {
                    for _result in results {
                        // TODO: Add to database
                    }
                }
                Err(e) => eprintln!("Error reading file: {:?} - {:?}", file_clone, e),
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
