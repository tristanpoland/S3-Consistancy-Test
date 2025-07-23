//! S3 Consistency Test Tool
//! 
//! A comprehensive tool for measuring eventual consistency propagation times
//! in S3-compatible storage systems. This tool uploads test files and measures
//! how long it takes for them to become consistently readable.
//! 
//! # Features
//! 
//! - Support for any S3-compatible storage (AWS S3, MinIO, DigitalOcean Spaces, etc.)
//! - Precise timing measurements with millisecond accuracy
//! - Comprehensive statistical analysis (percentiles, averages, distribution)
//! - Robust cleanup with retry logic and emergency cleanup on interruption
//! - Detailed JSON reporting for further analysis
//! - Real-time progress monitoring with structured logging
//! 
//! # Usage
//! 
//! ```bash
//! # Basic usage
//! s3-consistency-test --config config.toml
//! 
//! # Advanced usage with custom parameters
//! s3-consistency-test --config config.toml \
//!   --test-count 20 \
//!   --file-size 2048 \
//!   --max-wait 120 \
//!   --interval 50 \
//!   --verbose
//! ```
//! 
//! # Configuration
//! 
//! Create a `config.toml` file with your S3 settings:
//! 
//! ```toml
//! endpoint = "https://s3.amazonaws.com"
//! region = "us-east-1"
//! bucket = "your-test-bucket"
//! access_key = "your-access-key"
//! secret_key = "your-secret-key"
//! path_style = false
//! ```

mod cleanup;
mod config;
mod statistics;
mod tester;
mod types;

use anyhow::Result;
use clap::Parser;
use tracing::{error, info};

use crate::cleanup::setup_cleanup_handler;
use crate::config::load_config;
use crate::statistics::print_summary;
use crate::tester::S3ConsistencyTester;
use crate::types::Args;

/// Main application entry point
/// 
/// Orchestrates the complete S3 consistency testing process:
/// 1. Parse command-line arguments
/// 2. Initialize logging system
/// 3. Load S3 configuration
/// 4. Set up cleanup handlers for graceful shutdown
/// 5. Run consistency tests
/// 6. Generate and save detailed report
/// 
/// # Error Handling
/// 
/// The application uses structured error handling with the `anyhow` crate.
/// All errors are propagated up and displayed with context to help users
/// diagnose configuration or connection issues.
/// 
/// # Signal Handling
/// 
/// The application sets up a Ctrl+C handler to ensure cleanup of test files
/// even if the program is interrupted. This prevents leaving orphaned files
/// in the S3 bucket.
/// 
/// # Returns
/// 
/// - `Ok(())` if all tests complete successfully and report is saved
/// - `Err(anyhow::Error)` if any critical error occurs during execution

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command-line arguments
    let args = Args::parse();
    
    // Initialize structured logging
    initialize_logging(args.verbose);
    
    info!("🚀 S3 Consistency Test Tool starting...");
    
    // Load and validate S3 configuration
    let config = load_config(&args.config)
        .map_err(|e| {
            error!("Failed to load configuration: {}", e);
            e
        })?;
    
    info!("📋 Configuration loaded successfully");
    info!("🔗 Endpoint: {}", config.endpoint);
    info!("🪣 Bucket: {}", config.bucket);
    
    // Create the S3 consistency tester
    let tester = S3ConsistencyTester::new(config).await
        .map_err(|e| {
            error!("Failed to initialize S3 tester: {}", e);
            e
        })?;
    
    // Set up cleanup handler for graceful shutdown on interruption
    setup_cleanup_handler(tester.cleanup_manager());
    
    info!("🛡️  Cleanup handler configured for graceful shutdown");
    
    // Run the consistency test suite
    let report = tester.run_consistency_test(&args).await
        .map_err(|e| {
            error!("Consistency test failed: {}", e);
            e
        })?;
    
    // Display comprehensive summary to the user
    print_summary(&report);
    
    // Save detailed JSON report for further analysis
    let report_file = generate_report_filename(&report);
    save_json_report(&report, &report_file)?;
    
    info!("📊 Test completed successfully!");
    
    Ok(())
}

/// Initializes the logging system with appropriate verbosity
/// 
/// Sets up structured logging using the `tracing` crate with different
/// levels based on the verbose flag. Debug level provides detailed
/// information about each operation, while info level shows progress.
/// 
/// # Arguments
/// 
/// * `verbose` - Whether to enable debug-level logging
fn initialize_logging(verbose: bool) {
    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_ansi(true)
        .init();
}

/// Generates a timestamped filename for the JSON report
/// 
/// Creates a filename with the format `consistency-report-YYYYMMDD-HHMMSS.json`
/// based on the test start time to ensure unique filenames and easy sorting.
/// 
/// # Arguments
/// 
/// * `report` - The consistency report containing timing information
/// 
/// # Returns
/// 
/// A formatted filename string
fn generate_report_filename(report: &crate::types::ConsistencyReport) -> String {
    format!(
        "consistency-report-{}.json",
        report.test_start_time.format("%Y%m%d-%H%M%S")
    )
}

/// Saves the consistency report as a formatted JSON file
/// 
/// Serializes the complete report to pretty-formatted JSON and writes
/// it to disk. The JSON format allows for easy analysis with external
/// tools and provides a permanent record of test results.
/// 
/// # Arguments
/// 
/// * `report` - The consistency report to save
/// * `filename` - The filename to write to
/// 
/// # Returns
/// 
/// - `Ok(())` if the file was saved successfully
/// - `Err(anyhow::Error)` if serialization or file writing fails
fn save_json_report(report: &crate::types::ConsistencyReport, filename: &str) -> Result<()> {
    let report_json = serde_json::to_string_pretty(report)
        .map_err(|e| anyhow::anyhow!("Failed to serialize report to JSON: {}", e))?;
    
    std::fs::write(filename, report_json)
        .map_err(|e| anyhow::anyhow!("Failed to write report file {}: {}", filename, e))?;
    
    info!("💾 Detailed report saved to: {}", filename);
    
    Ok(())
}
