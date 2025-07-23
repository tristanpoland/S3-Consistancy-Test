//! Data types and structures for S3 consistency testing
//! 
//! This module defines all the data structures used throughout the S3 consistency
//! testing application, including test results, reports, statistics, and CLI arguments.

use chrono::{DateTime, Utc};
use clap::Parser;
use serde::Serialize;
use std::path::PathBuf;

use crate::config::S3Config;

/// Command-line arguments for the S3 consistency tester
/// 
/// This structure defines all command-line options available to users.
/// It uses the `clap` crate for argument parsing with helpful descriptions
/// and sensible defaults.
/// 
/// # Examples
/// 
/// ```bash
/// # Basic usage with config file
/// s3-consistency-test --config config.toml
/// 
/// # Advanced usage with custom parameters
/// s3-consistency-test --config config.toml \
///   --test-count 50 \
///   --file-size 2048 \
///   --max-wait 120 \
///   --interval 200 \
///   --verbose
/// ```
#[derive(Parser, Debug)]
#[command(name = "s3-consistency-test")]
#[command(about = "Test S3 eventual consistency propagation times")]
#[command(long_about = r#"
S3 Consistency Tester

This tool uploads test files to an S3-compatible storage service and measures
how long it takes for the files to become consistently readable across the
storage infrastructure. This helps identify eventual consistency behavior
and measure propagation times.

The tool supports any S3-compatible storage service including:
- Amazon S3
- MinIO
- DigitalOcean Spaces  
- Cloudflare R2
- Google Cloud Storage (S3 API)
- And many others

Results are displayed in real-time and saved to a detailed JSON report.
"#)]
pub struct Args {
    /// Path to the TOML configuration file
    /// 
    /// The configuration file must contain S3 connection details including
    /// endpoint, credentials, and bucket information.
    #[arg(short, long, help = "Path to configuration file")]
    pub config: PathBuf,
    
    /// Number of test files to upload and test
    /// 
    /// Each test file is uploaded sequentially and tested for consistency.
    /// More files provide better statistical accuracy but take longer to run.
    #[arg(short, long, default_value = "10", help = "Number of test files to upload")]
    pub test_count: usize,
    
    /// Size of each test file in bytes
    /// 
    /// Larger files may take longer to propagate and upload.
    /// Files are filled with random data to ensure uniqueness.
    #[arg(short, long, default_value = "1024", help = "Size of test files in bytes")]
    pub file_size: usize,
    
    /// Maximum time to wait for consistency in seconds
    /// 
    /// If a file hasn't become consistent within this time,
    /// the test for that file is marked as failed.
    #[arg(short, long, default_value = "300", help = "Maximum time to wait for consistency (seconds)")]
    pub max_wait: u64,
    
    /// Check interval between read attempts in milliseconds
    /// 
    /// How often to check if the uploaded file has become readable.
    /// Shorter intervals provide more precise timing but generate more load.
    #[arg(short, long, default_value = "100", help = "Check interval in milliseconds")]
    pub interval: u64,
    
    /// Enable verbose logging
    /// 
    /// Shows detailed debug information including individual read attempts,
    /// upload progress, and cleanup operations.
    #[arg(short, long, help = "Enable verbose logging")]
    pub verbose: bool,
}

/// Test parameters used during the consistency test
/// 
/// This structure captures the key parameters used during testing
/// for inclusion in the final report. It provides context for
/// interpreting the test results.
#[derive(Debug, Clone, Serialize)]
pub struct TestParameters {
    /// Number of test files that were uploaded
    pub test_count: usize,
    
    /// Size of each test file in bytes
    pub file_size: usize,
    
    /// Maximum wait time in seconds
    pub max_wait_seconds: u64,
    
    /// Check interval in milliseconds
    pub check_interval_ms: u64,
}

impl From<&Args> for TestParameters {
    fn from(args: &Args) -> Self {
        Self {
            test_count: args.test_count,
            file_size: args.file_size,
            max_wait_seconds: args.max_wait,
            check_interval_ms: args.interval,
        }
    }
}

/// Result of testing a single file for consistency
/// 
/// Contains detailed information about one consistency test,
/// including timing data, success status, and error details.
/// 
/// # Fields
/// 
/// - `file_key`: The S3 object key that was tested
/// - `upload_time`: When the file was successfully uploaded
/// - `first_read_success_time`: When the file first became readable (if successful)
/// - `propagation_duration_ms`: How long it took to become consistent in milliseconds
/// - `total_attempts`: Number of read attempts made
/// - `success`: Whether the consistency test succeeded
/// - `error_details`: Error message if the test failed
#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    /// The S3 object key that was tested
    /// 
    /// Generated as "consistency-test-{uuid}" to ensure uniqueness
    pub file_key: String,
    
    /// Timestamp when the file upload completed successfully
    /// 
    /// This is the baseline time used to calculate propagation duration
    pub upload_time: DateTime<Utc>,
    
    /// Timestamp when the file first became readable
    /// 
    /// `None` if the file never became consistently readable within the timeout
    pub first_read_success_time: Option<DateTime<Utc>>,
    
    /// Time in milliseconds from upload to first successful read
    /// 
    /// Calculated as `first_read_success_time - upload_time`.
    /// `None` if the consistency test failed or timed out.
    pub propagation_duration_ms: Option<u64>,
    
    /// Total number of read attempts made during the test
    /// 
    /// Includes both failed and the final successful attempt.
    /// Helps understand the load generated during testing.
    pub total_attempts: u32,
    
    /// Whether the consistency test completed successfully
    /// 
    /// `true` if the file became readable within the timeout,
    /// `false` if it failed or timed out.
    pub success: bool,
    
    /// Error message if the test failed
    /// 
    /// Contains details about what went wrong, such as:
    /// - Upload failures
    /// - Timeout errors  
    /// - Network errors
    /// - Authentication failures
    pub error_details: Option<String>,
}

impl TestResult {
    /// Creates a new successful test result
    /// 
    /// # Arguments
    /// 
    /// * `file_key` - The S3 object key that was tested
    /// * `upload_time` - When the upload completed
    /// * `first_read_success_time` - When the file became readable
    /// * `total_attempts` - Number of read attempts made
    /// 
    /// # Returns
    /// 
    /// A `TestResult` with success=true and calculated propagation duration
    pub fn success(
        file_key: String,
        upload_time: DateTime<Utc>,
        first_read_success_time: DateTime<Utc>,
        total_attempts: u32,
    ) -> Self {
        let propagation_duration_ms = first_read_success_time
            .signed_duration_since(upload_time)
            .num_milliseconds() as u64;
        
        Self {
            file_key,
            upload_time,
            first_read_success_time: Some(first_read_success_time),
            propagation_duration_ms: Some(propagation_duration_ms),
            total_attempts,
            success: true,
            error_details: None,
        }
    }
    
    /// Creates a new failed test result
    /// 
    /// # Arguments
    /// 
    /// * `file_key` - The S3 object key that was tested
    /// * `upload_time` - When the upload completed (or when it was attempted)
    /// * `error_details` - Description of what went wrong
    /// 
    /// # Returns
    /// 
    /// A `TestResult` with success=false and error details
    pub fn failure(
        file_key: String,
        upload_time: DateTime<Utc>,
        error_details: String,
    ) -> Self {
        Self {
            file_key,
            upload_time,
            first_read_success_time: None,
            propagation_duration_ms: None,
            total_attempts: 0,
            success: false,
            error_details: Some(error_details),
        }
    }
}

/// Statistical analysis of consistency test results
/// 
/// Provides comprehensive statistics about the consistency behavior
/// observed during testing. Includes success rates and timing percentiles.
/// 
/// All timing values are in milliseconds for consistency.
#[derive(Debug, Clone, Serialize)]
pub struct ConsistencyStatistics {
    /// Number of tests that completed successfully
    pub successful_tests: usize,
    
    /// Number of tests that failed or timed out
    pub failed_tests: usize,
    
    /// Success rate as a percentage (0.0 to 100.0)
    pub success_rate: f64,
    
    /// Fastest propagation time observed (milliseconds)
    /// 
    /// `None` if no tests succeeded
    pub min_propagation_time_ms: Option<u64>,
    
    /// Slowest propagation time observed (milliseconds)
    /// 
    /// `None` if no tests succeeded
    pub max_propagation_time_ms: Option<u64>,
    
    /// Average propagation time (milliseconds)
    /// 
    /// `None` if no tests succeeded
    pub avg_propagation_time_ms: Option<f64>,
    
    /// Median propagation time (milliseconds)
    /// 
    /// The middle value when all successful propagation times are sorted.
    /// `None` if no tests succeeded.
    pub median_propagation_time_ms: Option<u64>,
    
    /// 95th percentile propagation time (milliseconds)
    /// 
    /// 95% of successful tests completed within this time.
    /// `None` if insufficient successful tests.
    pub percentile_95_ms: Option<u64>,
    
    /// 99th percentile propagation time (milliseconds)
    /// 
    /// 99% of successful tests completed within this time.
    /// `None` if insufficient successful tests.
    pub percentile_99_ms: Option<u64>,
}

/// Complete consistency test report
/// 
/// Contains all information about a consistency test run, including
/// configuration, parameters, individual results, and statistics.
/// This structure is serialized to JSON for detailed reporting.
#[derive(Debug, Serialize)]
pub struct ConsistencyReport {
    /// When the test suite started
    pub test_start_time: DateTime<Utc>,
    
    /// When the test suite completed
    pub test_end_time: DateTime<Utc>,
    
    /// Total time taken for all tests (milliseconds)
    pub total_duration_ms: u64,
    
    /// S3 configuration used for testing
    /// 
    /// Includes endpoint, bucket, and other connection details.
    /// Credentials are included for debugging but should be redacted
    /// before sharing reports.
    pub config: S3Config,
    
    /// Parameters used during testing
    pub test_parameters: TestParameters,
    
    /// Individual test results for each file
    /// 
    /// Results are in the order the tests were performed.
    pub results: Vec<TestResult>,
    
    /// Statistical summary of the results
    pub statistics: ConsistencyStatistics,
}

impl ConsistencyReport {
    /// Creates a new consistency report
    /// 
    /// # Arguments
    /// 
    /// * `test_start_time` - When testing began
    /// * `test_end_time` - When testing completed
    /// * `config` - S3 configuration used
    /// * `test_parameters` - Test parameters used
    /// * `results` - Individual test results
    /// * `statistics` - Calculated statistics
    /// 
    /// # Returns
    /// 
    /// A new `ConsistencyReport` with calculated total duration
    pub fn new(
        test_start_time: DateTime<Utc>,
        test_end_time: DateTime<Utc>,
        config: S3Config,
        test_parameters: TestParameters,
        results: Vec<TestResult>,
        statistics: ConsistencyStatistics,
    ) -> Self {
        let total_duration_ms = test_end_time
            .signed_duration_since(test_start_time)
            .num_milliseconds() as u64;
        
        Self {
            test_start_time,
            test_end_time,
            total_duration_ms,
            config,
            test_parameters,
            results,
            statistics,
        }
    }
    
    /// Gets a summary of the test results as a formatted string
    /// 
    /// Returns a human-readable summary including success rate,
    /// timing statistics, and other key metrics.
    /// 
    /// # Returns
    /// 
    /// A formatted string suitable for console output
    pub fn get_summary(&self) -> String {
        let stats = &self.statistics;
        
        let mut summary = format!(
            "Test completed in {}ms\n",
            self.total_duration_ms
        );
        
        summary.push_str(&format!(
            "Success rate: {}/{} ({:.1}%)\n",
            stats.successful_tests,
            stats.successful_tests + stats.failed_tests,
            stats.success_rate
        ));
        
        if let Some(min) = stats.min_propagation_time_ms {
            summary.push_str(&format!("Min propagation time: {}ms\n", min));
        }
        
        if let Some(max) = stats.max_propagation_time_ms {
            summary.push_str(&format!("Max propagation time: {}ms\n", max));
        }
        
        if let Some(avg) = stats.avg_propagation_time_ms {
            summary.push_str(&format!("Avg propagation time: {:.1}ms\n", avg));
        }
        
        if let Some(median) = stats.median_propagation_time_ms {
            summary.push_str(&format!("Median propagation time: {}ms\n", median));
        }
        
        summary
    }
}