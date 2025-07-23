use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use rand::Rng;
use s3::creds::Credentials;
use s3::{Bucket, Region};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "s3-consistency-test")]
#[command(about = "Test S3 eventual consistency propagation times")]
struct Args {
    #[arg(short, long, help = "Path to configuration file")]
    config: PathBuf,
    
    #[arg(short, long, default_value = "10", help = "Number of test files to upload")]
    test_count: usize,
    
    #[arg(short, long, default_value = "1024", help = "Size of test files in bytes")]
    file_size: usize,
    
    #[arg(short, long, default_value = "300", help = "Maximum time to wait for consistency (seconds)")]
    max_wait: u64,
    
    #[arg(short, long, default_value = "100", help = "Check interval in milliseconds")]
    interval: u64,
    
    #[arg(short, long, help = "Enable verbose logging")]
    verbose: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct S3Config {
    endpoint: String,
    region: String,
    bucket: String,
    access_key: String,
    secret_key: String,
    path_style: Option<bool>,
}

#[derive(Debug, Serialize)]
struct TestResult {
    file_key: String,
    upload_time: DateTime<Utc>,
    first_read_success_time: Option<DateTime<Utc>>,
    propagation_duration_ms: Option<u64>,
    total_attempts: u32,
    success: bool,
    error_details: Option<String>,
}

#[derive(Debug, Serialize)]
struct ConsistencyReport {
    test_start_time: DateTime<Utc>,
    test_end_time: DateTime<Utc>,
    total_duration_ms: u64,
    config: S3Config,
    test_parameters: TestParameters,
    results: Vec<TestResult>,
    statistics: ConsistencyStatistics,
}

#[derive(Debug, Serialize)]
struct TestParameters {
    test_count: usize,
    file_size: usize,
    max_wait_seconds: u64,
    check_interval_ms: u64,
}

#[derive(Debug, Serialize)]
struct ConsistencyStatistics {
    successful_tests: usize,
    failed_tests: usize,
    success_rate: f64,
    min_propagation_time_ms: Option<u64>,
    max_propagation_time_ms: Option<u64>,
    avg_propagation_time_ms: Option<f64>,
    median_propagation_time_ms: Option<u64>,
    percentile_95_ms: Option<u64>,
    percentile_99_ms: Option<u64>,
}

struct S3ConsistencyTester {
    bucket: Bucket,
    config: S3Config,
    active_test_files: Arc<Mutex<Vec<String>>>,
}

impl S3ConsistencyTester {
    async fn new(config: S3Config) -> Result<Self> {
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )?;

        let region = if config.endpoint.contains("amazonaws.com") {
            Region::from_str(&config.region)?
        } else {
            Region::Custom {
                region: config.region.clone(),
                endpoint: config.endpoint.clone(),
            }
        };

        let mut bucket = Bucket::new(&config.bucket, region, credentials)?;
        
        if config.path_style.unwrap_or(false) {
            bucket = bucket.with_path_style();
        }

        Ok(Self { 
            bucket, 
            config,
            active_test_files: Arc::new(Mutex::new(Vec::new())),
        })
    }

    async fn test_consistency(&self, args: &Args) -> Result<ConsistencyReport> {
        let test_start = Utc::now();
        let start_instant = Instant::now();
        
        info!("Starting S3 consistency test with {} files", args.test_count);
        info!("S3 endpoint: {}", self.config.endpoint);
        info!("Bucket: {}", self.config.bucket);
        info!("File size: {} bytes", args.file_size);
        info!("Max wait time: {} seconds", args.max_wait);
        info!("Check interval: {} ms", args.interval);

        let mut results = Vec::new();

        for i in 0..args.test_count {
            info!("Testing file {}/{}", i + 1, args.test_count);
            
            let test_result = self.test_single_file(args).await;
            results.push(test_result);
            
            // Small delay between tests to avoid overwhelming the server
            sleep(Duration::from_millis(100)).await;
        }

        let test_end = Utc::now();
        let total_duration = start_instant.elapsed();

        let statistics = self.calculate_statistics(&results);
        
        let report = ConsistencyReport {
            test_start_time: test_start,
            test_end_time: test_end,
            total_duration_ms: total_duration.as_millis() as u64,
            config: self.config.clone(),
            test_parameters: TestParameters {
                test_count: args.test_count,
                file_size: args.file_size,
                max_wait_seconds: args.max_wait,
                check_interval_ms: args.interval,
            },
            results,
            statistics,
        };

        self.print_summary(&report);
        Ok(report)
    }

    async fn test_single_file(&self, args: &Args) -> TestResult {
        let file_key = format!("consistency-test-{}", Uuid::new_v4());
        let test_data = self.generate_test_data(args.file_size);
        let upload_time = Utc::now();
        
        debug!("Uploading test file: {}", file_key);
        
        // Upload the file
        match self.bucket.put_object(&file_key, &test_data).await {
            Ok(_) => {
                debug!("Successfully uploaded {}", file_key);
                
                // Track active test file for cleanup
                {
                    let mut active_files = self.active_test_files.lock().await;
                    active_files.push(file_key.clone());
                }
                
                // Now test for consistency by repeatedly trying to read the file
                let consistency_result = self.test_read_consistency(&file_key, args).await;
                
                // Always clean up the test file, regardless of test outcome
                self.cleanup_test_file(&file_key).await;
                
                // Remove from active tracking
                {
                    let mut active_files = self.active_test_files.lock().await;
                    active_files.retain(|f| f != &file_key);
                }
                
                match consistency_result {
                    Ok((first_success_time, attempts)) => {
                        let propagation_duration = first_success_time
                            .signed_duration_since(upload_time)
                            .num_milliseconds() as u64;
                        
                        TestResult {
                            file_key,
                            upload_time,
                            first_read_success_time: Some(first_success_time),
                            propagation_duration_ms: Some(propagation_duration),
                            total_attempts: attempts,
                            success: true,
                            error_details: None,
                        }
                    }
                    Err(e) => {
                        error!("Consistency test failed for {}: {}", file_key, e);
                        TestResult {
                            file_key,
                            upload_time,
                            first_read_success_time: None,
                            propagation_duration_ms: None,
                            total_attempts: 0,
                            success: false,
                            error_details: Some(e.to_string()),
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to upload test file {}: {}", file_key, e);
                TestResult {
                    file_key,
                    upload_time,
                    first_read_success_time: None,
                    propagation_duration_ms: None,
                    total_attempts: 0,
                    success: false,
                    error_details: Some(format!("Upload failed: {}", e)),
                }
            }
        }
    }

    async fn cleanup_test_file(&self, file_key: &str) {
        debug!("Cleaning up test file: {}", file_key);
        
        // Try multiple times to ensure cleanup succeeds
        for attempt in 1..=3 {
            match self.bucket.delete_object(file_key).await {
                Ok(_) => {
                    debug!("Successfully cleaned up test file: {}", file_key);
                    return;
                }
                Err(e) => {
                    if attempt == 3 {
                        error!("Failed to clean up test file {} after {} attempts: {}", file_key, attempt, e);
                    } else {
                        warn!("Cleanup attempt {} failed for {}: {}, retrying...", attempt, file_key, e);
                        sleep(Duration::from_millis(1000)).await;
                    }
                }
            }
        }
    }

    async fn test_read_consistency(&self, file_key: &str, args: &Args) -> Result<(DateTime<Utc>, u32)> {
        let max_duration = Duration::from_secs(args.max_wait);
        let check_interval = Duration::from_millis(args.interval);
        let start_time = Instant::now();
        let mut attempts = 0;

        debug!("Starting consistency check for {}", file_key);

        loop {
            attempts += 1;
            
            match timeout(Duration::from_secs(5), self.bucket.get_object(file_key)).await {
                Ok(Ok(response)) => {
                    let success_time = Utc::now();
                    let elapsed = start_time.elapsed();
                    
                    info!(
                        "File {} became consistent after {} attempts in {}ms",
                        file_key,
                        attempts,
                        elapsed.as_millis()
                    );
                    
                    debug!("Response status: {}", response.status_code());
                    return Ok((success_time, attempts));
                }
                Ok(Err(e)) => {
                    debug!("Attempt {} failed for {}: {}", attempts, file_key, e);
                }
                Err(_) => {
                    debug!("Attempt {} timed out for {}", attempts, file_key);
                }
            }

            if start_time.elapsed() >= max_duration {
                return Err(anyhow::anyhow!(
                    "Consistency test timed out after {} attempts in {}ms",
                    attempts,
                    max_duration.as_millis()
                ));
            }

            sleep(check_interval).await;
        }
    }

    async fn cleanup_all_test_files(&self) {
        let active_files = {
            let active_files_guard = self.active_test_files.lock().await;
            active_files_guard.clone()
        };
        
        if !active_files.is_empty() {
            info!("Cleaning up {} remaining test files...", active_files.len());
            
            for file_key in active_files {
                self.cleanup_test_file(&file_key).await;
            }
            
            // Clear the list
            {
                let mut active_files_guard = self.active_test_files.lock().await;
                active_files_guard.clear();
            }
            
            info!("Emergency cleanup completed");
        }
    }

    fn generate_test_data(&self, size: usize) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut data = vec![0u8; size];
        rng.fill(&mut data[..]);
        data
    }

    fn calculate_statistics(&self, results: &[TestResult]) -> ConsistencyStatistics {
        let successful_results: Vec<_> = results
            .iter()
            .filter(|r| r.success && r.propagation_duration_ms.is_some())
            .collect();

        let successful_tests = successful_results.len();
        let failed_tests = results.len() - successful_tests;
        let success_rate = if results.is_empty() {
            0.0
        } else {
            successful_tests as f64 / results.len() as f64 * 100.0
        };

        if successful_results.is_empty() {
            return ConsistencyStatistics {
                successful_tests,
                failed_tests,
                success_rate,
                min_propagation_time_ms: None,
                max_propagation_time_ms: None,
                avg_propagation_time_ms: None,
                median_propagation_time_ms: None,
                percentile_95_ms: None,
                percentile_99_ms: None,
            };
        }

        let mut durations: Vec<u64> = successful_results
            .iter()
            .map(|r| r.propagation_duration_ms.unwrap())
            .collect();
        durations.sort_unstable();

        let min_propagation_time_ms = durations.first().copied();
        let max_propagation_time_ms = durations.last().copied();
        let avg_propagation_time_ms = Some(durations.iter().sum::<u64>() as f64 / durations.len() as f64);
        
        let median_propagation_time_ms = if durations.len() % 2 == 0 {
            Some((durations[durations.len() / 2 - 1] + durations[durations.len() / 2]) / 2)
        } else {
            Some(durations[durations.len() / 2])
        };

        let percentile_95_ms = durations.get((durations.len() as f64 * 0.95) as usize).copied();
        let percentile_99_ms = durations.get((durations.len() as f64 * 0.99) as usize).copied();

        ConsistencyStatistics {
            successful_tests,
            failed_tests,
            success_rate,
            min_propagation_time_ms,
            max_propagation_time_ms,
            avg_propagation_time_ms,
            median_propagation_time_ms,
            percentile_95_ms,
            percentile_99_ms,
        }
    }

    fn print_summary(&self, report: &ConsistencyReport) {
        println!("\n=== S3 Consistency Test Summary ===");
        println!("Test Duration: {}ms", report.total_duration_ms);
        println!("Endpoint: {}", report.config.endpoint);
        println!("Bucket: {}", report.config.bucket);
        println!("Files Tested: {}", report.test_parameters.test_count);
        println!("File Size: {} bytes", report.test_parameters.file_size);
        println!();
        
        let stats = &report.statistics;
        println!("=== Results ===");
        println!("Successful Tests: {} ({:.1}%)", stats.successful_tests, stats.success_rate);
        println!("Failed Tests: {}", stats.failed_tests);
        println!();
        
        if stats.successful_tests > 0 {
            println!("=== Propagation Times ===");
            if let Some(min) = stats.min_propagation_time_ms {
                println!("Minimum: {}ms", min);
            }
            if let Some(max) = stats.max_propagation_time_ms {
                println!("Maximum: {}ms", max);
            }
            if let Some(avg) = stats.avg_propagation_time_ms {
                println!("Average: {:.1}ms", avg);
            }
            if let Some(median) = stats.median_propagation_time_ms {
                println!("Median: {}ms", median);
            }
            if let Some(p95) = stats.percentile_95_ms {
                println!("95th Percentile: {}ms", p95);
            }
            if let Some(p99) = stats.percentile_99_ms {
                println!("99th Percentile: {}ms", p99);
            }
        }
        
        println!("\n=== Individual Test Results ===");
        for (i, result) in report.results.iter().enumerate() {
            print!("Test {}: ", i + 1);
            if result.success {
                if let Some(duration) = result.propagation_duration_ms {
                    println!("SUCCESS - {}ms ({} attempts)", duration, result.total_attempts);
                } else {
                    println!("SUCCESS - immediate");
                }
            } else {
                println!("FAILED - {}", result.error_details.as_deref().unwrap_or("Unknown error"));
            }
        }
    }
}

fn load_config(path: &PathBuf) -> Result<S3Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    
    let config: S3Config = toml::from_str(&content)
        .with_context(|| "Failed to parse config file as TOML")?;
    
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize tracing
    let level = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .init();

    // Load configuration
    let config = load_config(&args.config)?;
    
    // Create tester and run tests
    let tester = Arc::new(S3ConsistencyTester::new(config).await?);
    
    // Set up cleanup handler for Ctrl+C
    let tester_cleanup = tester.clone();
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            warn!("Received interrupt signal, cleaning up test files...");
            tester_cleanup.cleanup_all_test_files().await;
            std::process::exit(1);
        }
    });
    
    let report = tester.test_consistency(&args).await?;
    
    // Final cleanup check (should be empty, but just in case)
    tester.cleanup_all_test_files().await;
    
    // Save detailed report to JSON file
    let report_file = format!("consistency-report-{}.json", Utc::now().format("%Y%m%d-%H%M%S"));
    let report_json = serde_json::to_string_pretty(&report)?;
    std::fs::write(&report_file, report_json)?;
    
    println!("\nDetailed report saved to: {}", report_file);
    
    Ok(())
}
