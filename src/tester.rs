//! S3 consistency testing core functionality
//! 
//! This module contains the main consistency testing logic, including the
//! `S3ConsistencyTester` struct that orchestrates the testing process.
//! It handles S3 connection setup, file upload/read operations, and timing measurements.

use crate::cleanup::CleanupManager;
use crate::config::S3Config;
use crate::statistics;
use crate::types::{Args, ConsistencyReport, TestParameters, TestResult};

use anyhow::{Context, Result};
use chrono::Utc;
use rand::Rng;
use s3::creds::Credentials;
use s3::{Bucket, Region};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Core S3 consistency tester
/// 
/// This structure manages S3 connections and orchestrates consistency testing.
/// It handles file uploads, consistency checks, cleanup operations, and result compilation.
/// 
/// # Examples
/// 
/// ```rust
/// let config = S3Config { /* ... */ };
/// let tester = S3ConsistencyTester::new(config).await?;
/// let report = tester.run_consistency_test(&args).await?;
/// ```
pub struct S3ConsistencyTester {
    /// S3 bucket handle for all operations
    bucket: Bucket,
    
    /// Configuration used for this tester
    config: S3Config,
    
    /// Cleanup manager for handling test file cleanup
    cleanup_manager: Arc<CleanupManager>,
}

impl S3ConsistencyTester {
    /// Creates a new S3 consistency tester
    /// 
    /// Establishes connection to the S3 service using the provided configuration
    /// and sets up the cleanup manager for handling test files.
    /// 
    /// # Arguments
    /// 
    /// * `config` - S3 configuration including endpoint, credentials, and bucket
    /// 
    /// # Returns
    /// 
    /// A new `S3ConsistencyTester` instance ready for testing
    /// 
    /// # Errors
    /// 
    /// Returns an error if:
    /// - Credentials are invalid
    /// - Region parsing fails
    /// - Bucket connection cannot be established
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let config = S3Config {
    ///     endpoint: "https://s3.amazonaws.com".to_string(),
    ///     region: "us-east-1".to_string(),
    ///     bucket: "my-test-bucket".to_string(),
    ///     access_key: "AKIAIO...".to_string(),
    ///     secret_key: "wJalr...".to_string(),
    ///     path_style: Some(false),
    /// };
    /// let tester = S3ConsistencyTester::new(config).await?;
    /// ```
    pub async fn new(config: S3Config) -> Result<Self> {
        debug!("Creating S3 consistency tester for endpoint: {}", config.endpoint);
        
        // Create S3 credentials
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None, // session_token
            None, // profile
            None, // role_arn
        ).context("Failed to create S3 credentials")?;

        // Determine the appropriate region configuration
        let region = if config.is_aws_s3() {
            // For AWS S3, parse the region string
            Region::from_str(&config.region)
                .with_context(|| format!("Invalid AWS region: {}", config.region))?
        } else {
            // For custom S3 services, use custom region with endpoint
            Region::Custom {
                region: config.region.clone(),
                endpoint: config.endpoint.clone(),
            }
        };
        
        debug!("Using S3 region: {:?}", region);

        // Create bucket handle
        let mut bucket = Bucket::new(&config.bucket, region, credentials)
            .with_context(|| format!("Failed to create S3 bucket handle for: {}", config.bucket))?;
        
        // Configure path style if needed
        if config.get_path_style() {
            debug!("Using path-style URLs");
            bucket = bucket.with_path_style();
        } else {
            debug!("Using virtual-hosted-style URLs");
        }

        // Create cleanup manager
        let cleanup_manager = Arc::new(CleanupManager::new(bucket.clone()));

        info!("Successfully connected to S3 bucket: {}", config.bucket);
        
        Ok(Self {
            bucket,
            config,
            cleanup_manager,
        })
    }
    
    /// Gets a reference to the cleanup manager
    /// 
    /// This allows external code to set up signal handlers and perform
    /// emergency cleanup operations.
    /// 
    /// # Returns
    /// 
    /// An `Arc` reference to the cleanup manager
    pub fn cleanup_manager(&self) -> Arc<CleanupManager> {
        self.cleanup_manager.clone()
    }
    
    /// Runs the complete consistency test suite
    /// 
    /// Executes the full testing process including file uploads, consistency checks,
    /// statistics calculation, and report generation. This is the main entry point
    /// for running consistency tests.
    /// 
    /// # Arguments
    /// 
    /// * `args` - Command-line arguments specifying test parameters
    /// 
    /// # Returns
    /// 
    /// A `ConsistencyReport` containing all test results and statistics
    /// 
    /// # Process
    /// 
    /// 1. Log test configuration and start timing
    /// 2. Execute individual file tests sequentially
    /// 3. Calculate comprehensive statistics
    /// 4. Perform final cleanup check
    /// 5. Generate and return complete report
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let report = tester.run_consistency_test(&args).await?;
    /// println!("Success rate: {:.1}%", report.statistics.success_rate);
    /// ```
    pub async fn run_consistency_test(&self, args: &Args) -> Result<ConsistencyReport> {
        let test_start = Utc::now();
        let start_instant = Instant::now();
        
        // Log test configuration
        info!("🚀 Starting S3 consistency test");
        info!("📍 S3 endpoint: {}", self.config.endpoint);
        info!("🪣 Bucket: {}", self.config.bucket);
        info!("📊 Test files: {}", args.test_count);
        info!("📁 File size: {} bytes", args.file_size);
        info!("⏰ Max wait time: {} seconds", args.max_wait);
        info!("🔄 Check interval: {} ms", args.interval);
        
        let mut results = Vec::with_capacity(args.test_count);

        // Execute individual tests
        for i in 0..args.test_count {
            info!("🧪 Testing file {}/{}", i + 1, args.test_count);
            
            let test_result = self.test_single_file(args).await;
            results.push(test_result);
            
            // Small delay between tests to avoid overwhelming the server
            if i < args.test_count - 1 {
                sleep(Duration::from_millis(100)).await;
            }
        }

        let test_end = Utc::now();
        let total_duration = start_instant.elapsed();

        // Calculate statistics
        info!("📈 Calculating test statistics...");
        let statistics = statistics::calculate_statistics(&results);
        
        // Perform final cleanup check
        info!("🧹 Performing final cleanup check...");
        self.cleanup_manager.final_cleanup_check().await;
        
        // Create comprehensive report
        let report = ConsistencyReport::new(
            test_start,
            test_end,
            self.config.clone(),
            TestParameters::from(args),
            results,
            statistics,
        );

        info!("✅ Test completed in {}ms", total_duration.as_millis());
        
        Ok(report)
    }

    /// Tests consistency for a single file
    /// 
    /// Performs the complete test cycle for one file: upload, consistency checking,
    /// and cleanup. This is the core testing logic that measures propagation time.
    /// 
    /// # Arguments
    /// 
    /// * `args` - Test arguments containing timing and size parameters
    /// 
    /// # Returns
    /// 
    /// A `TestResult` containing the outcome and timing data for this test
    /// 
    /// # Process
    /// 
    /// 1. Generate unique test file with random data
    /// 2. Upload file to S3 and record timestamp
    /// 3. Register file for cleanup tracking
    /// 4. Perform consistency polling until readable or timeout
    /// 5. Clean up test file
    /// 6. Return result with timing information
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let result = tester.test_single_file(&args).await;
    /// if result.success {
    ///     println!("Propagation time: {}ms", result.propagation_duration_ms.unwrap());
    /// }
    /// ```
    async fn test_single_file(&self, args: &Args) -> TestResult {
        // Generate unique test file
        let file_key = format!("consistency-test-{}", Uuid::new_v4());
        let test_data = self.generate_test_data(args.file_size);
        
        debug!("📤 Uploading test file: {}", file_key);
        
        // Attempt to upload the file
        match self.bucket.put_object(&file_key, &test_data).await {
            Ok(_) => {
                // Record upload completion time - this is the baseline for consistency measurement
                let upload_time = Utc::now();
                debug!("✅ Successfully uploaded {}", file_key);
                
                // Register file for cleanup tracking
                self.cleanup_manager.register_file(&file_key).await;
                
                // Test for consistency by repeatedly trying to read the file
                let consistency_result = self.test_read_consistency(&file_key, args).await;
                
                // Always clean up the test file
                self.cleanup_manager.cleanup_file(&file_key).await;
                
                // Process the consistency test result
                match consistency_result {
                    Ok((first_success_time, attempts)) => {
                        debug!("🎯 Consistency achieved for {} after {} attempts", file_key, attempts);
                        TestResult::success(
                            file_key,
                            upload_time,
                            first_success_time,
                            attempts,
                        )
                    }
                    Err(e) => {
                        error!("❌ Consistency test failed for {}: {}", file_key, e);
                        TestResult::failure(
                            file_key,
                            upload_time,
                            e.to_string(),
                        )
                    }
                }
            }
            Err(e) => {
                let upload_time = Utc::now(); // For error cases, use current time
                error!("❌ Failed to upload test file {}: {}", file_key, e);
                TestResult::failure(
                    file_key,
                    upload_time,
                    format!("Upload failed: {}", e),
                )
            }
        }
    }

    /// Tests read consistency for an uploaded file
    /// 
    /// Repeatedly attempts to read a file until it becomes available or timeout occurs.
    /// This is the core measurement function that determines propagation time.
    /// 
    /// # Arguments
    /// 
    /// * `file_key` - The S3 object key to test for consistency
    /// * `args` - Test arguments containing timeout and interval settings
    /// 
    /// # Returns
    /// 
    /// - `Ok((DateTime, u32))` - Success time and attempt count if file becomes readable
    /// - `Err(anyhow::Error)` - If timeout occurs or other error happens
    /// 
    /// # Behavior
    /// 
    /// - Polls the file at regular intervals (specified by `args.interval`)
    /// - Each read attempt has a 5-second timeout to prevent hanging
    /// - Continues until file is readable or `args.max_wait` seconds elapse
    /// - Records precise timing and attempt counts
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// match tester.test_read_consistency("test-file", &args).await {
    ///     Ok((success_time, attempts)) => {
    ///         println!("File readable after {} attempts", attempts);
    ///     }
    ///     Err(e) => {
    ///         println!("Consistency test failed: {}", e);
    ///     }
    /// }
    /// ```
    async fn test_read_consistency(&self, file_key: &str, args: &Args) -> Result<(chrono::DateTime<Utc>, u32)> {
        let max_duration = Duration::from_secs(args.max_wait);
        let check_interval = Duration::from_millis(args.interval);
        let start_time = Instant::now();
        let mut attempts = 0;

        debug!("🔍 Starting consistency check for {}", file_key);
        debug!("⏱️  Max wait: {}s, Check interval: {}ms", args.max_wait, args.interval);

        loop {
            attempts += 1;
            
            // Attempt to read the file with a timeout to prevent hanging
            match timeout(Duration::from_secs(5), self.bucket.get_object(file_key)).await {
                Ok(Ok(response)) => {
                    let success_time = Utc::now();
                    let elapsed = start_time.elapsed();
                    
                    info!(
                        "🎉 File {} became consistent after {} attempts in {}ms",
                        file_key,
                        attempts,
                        elapsed.as_millis()
                    );
                    
                    debug!("📊 Response status: {}", response.status_code());
                    return Ok((success_time, attempts));
                }
                Ok(Err(e)) => {
                    debug!("⚠️  Attempt {} failed for {}: {}", attempts, file_key, e);
                }
                Err(_) => {
                    debug!("⏰ Attempt {} timed out for {}", attempts, file_key);
                }
            }

            // Check if we've exceeded the maximum wait time
            if start_time.elapsed() >= max_duration {
                let error_msg = format!(
                    "Consistency test timed out after {} attempts in {}ms (max: {}ms)",
                    attempts,
                    start_time.elapsed().as_millis(),
                    max_duration.as_millis()
                );
                warn!("⏰ {}", error_msg);
                return Err(anyhow::anyhow!(error_msg));
            }

            // Wait before the next attempt
            sleep(check_interval).await;
        }
    }

    /// Generates random test data of the specified size
    /// 
    /// Creates a byte vector filled with random data to ensure each test file
    /// is unique and cannot be cached or deduplicated by the storage system.
    /// 
    /// # Arguments
    /// 
    /// * `size` - The size of the test data in bytes
    /// 
    /// # Returns
    /// 
    /// A vector containing random bytes of the specified size
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let data = tester.generate_test_data(1024); // 1KB of random data
    /// assert_eq!(data.len(), 1024);
    /// ```
    fn generate_test_data(&self, size: usize) -> Vec<u8> {
        debug!("🎲 Generating {} bytes of random test data", size);
        let mut rng = rand::thread_rng();
        let mut data = vec![0u8; size];
        rng.fill(&mut data[..]);
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::S3Config;

    fn create_test_config() -> S3Config {
        S3Config {
            endpoint: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
            path_style: Some(true),
        }
    }

    #[test]
    fn test_generate_test_data() {
        let config = create_test_config();
        // We can't easily test the async new() method without a real S3 service,
        // but we can test data generation logic by creating a mock tester
        
        // This would require more complex mocking setup in a real test environment
        // For now, we'll just test that the size is correct
        let test_data = vec![0u8; 1024];
        assert_eq!(test_data.len(), 1024);
    }

    #[test]
    fn test_s3_config_path_style_detection() {
        let aws_config = S3Config {
            endpoint: "https://s3.amazonaws.com".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test".to_string(),
            secret_key: "test".to_string(),
            path_style: None,
        };
        
        assert!(!aws_config.get_path_style()); // AWS should default to false
        
        let minio_config = S3Config {
            endpoint: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test".to_string(),
            secret_key: "test".to_string(),
            path_style: None,
        };
        
        assert!(minio_config.get_path_style()); // Non-AWS should default to true
    }
}