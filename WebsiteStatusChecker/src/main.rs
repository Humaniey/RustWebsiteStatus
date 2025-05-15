use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::{Duration, SystemTime};
use reqwest::blocking::Client;
use std::time::Instant;
use std::sync::mpsc;
use std::thread;


#[derive(Debug)]
struct WebsiteStatus {
    url: String,
    action_status: Result<u16, String>,
    response_time: Duration,
    timestamp: SystemTime,
}


fn read_urls_from_file(path: &str) -> Vec<String> {
    let file = File::open(path).expect("Failed to open file");
    let reader = BufReader::new(file);
    reader.lines()
        .filter_map(|line| {
            let line = line.ok()?.trim().to_string();
            if line.is_empty() || line.starts_with('#') { None } else { Some(line) }
        })
        .collect()
}


fn check_website(client: &Client, url: &str, timeout: Duration, retries: usize) -> WebsiteStatus {
    let start_time = Instant::now();
    let mut attempt = 0;

    loop {
        println!("Attempt {} for {}", attempt + 1, url);
        let result = client.get(url)
            .timeout(timeout)
            .send()
            .map(|res| res.status().as_u16())
            .map_err(|err| err.to_string());

        if result.is_ok() || attempt >= retries {
            return WebsiteStatus {
                url: url.to_string(),
                action_status: result,
                response_time: start_time.elapsed(),
                timestamp: SystemTime::now(),
            };
        }

        attempt += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
}


fn run_checks(urls: Vec<String>, workers: usize, timeout: Duration, retries: usize) -> Vec<WebsiteStatus> {
    let client = Client::new();
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();

    for chunk in urls.chunks(workers) {
        let tx = tx.clone();
        let client = client.clone();
        let chunk = chunk.to_vec();

        let handle = thread::spawn(move || {
            for url in chunk {
                let status = check_website(&client, &url, timeout, retries);
                println!("Sending result for {}", url);
                tx.send(status).expect("Failed to send");
            }
        });

        handles.push(handle);
    }

    println!("Waiting for workers to complete...");

    // Wait for all threads to finish
    for handle in handles {
        handle.join().unwrap();
    }

    drop(tx); // Closer the sender before collecting results

    println!("All workers finished processing!");

    // Collect results
    rx.iter().collect()
}


fn print_status(status: &WebsiteStatus) {
    match &status.action_status {
        Ok(code) => println!("[{}] {} ({:?})", status.url, code, status.response_time),
        Err(err) => println!("[{}] ERROR: {} ({:?})", status.url, err, status.response_time),
    }
}


fn save_results_to_json(statuses: &[WebsiteStatus]) -> String {
    let mut json_string = String::from("[");
    for status in statuses {
        json_string.push_str(&format!(
            r#"{{"url":"{}", "status":{}, "response_time":"{:?}", "timestamp":"{:?}"}},"#,
            status.url,
            match &status.action_status {
                Ok(code) => code.to_string(),
                Err(err) => format!(r#""{}""#, err),
            },
            status.response_time,
            status.timestamp
        ));
    }
    json_string.pop(); // Remove trailing comma
    json_string.push(']');
    json_string
}


fn main() {
    let urls = read_urls_from_file("urls.txt");
    let workers = 4;
    let timeout = Duration::from_secs(3);
    let retries = 0;

    let results = run_checks(urls, workers, timeout, retries);
    
    println!("\nWebsite Status Results:\n");
    
    for status in &results {
        print_status(status);  // Ensure this function prints each site's result  
    }

    let json_output = save_results_to_json(&results);  
    std::fs::write("status.json", json_output).expect("Failed to write JSON file");
}