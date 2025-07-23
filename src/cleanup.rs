//! File cleanup operations for S3 consistency testing
//! 
//! This module handles the cleanup of test files uploaded during consistency testing.
//! It provides robust cleanup capabilities including retry logic, emergency cleanup
//! for program interruption, and tracking of active test files.

use s3::Bucket;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Manages cleanup operations for S3 test files
/// 
/// This structure tracks active test files and provides methods for
/// cleaning them up reliably. It handles retry logic and provides
/// emergency cleanup capabilities for program interruption scenarios.
/// 
/// # Thread Safety
/// 
/// This structure is designed to be shared across async tasks using `Arc`.
/// The internal file tracking uses `Mutex` for thread-safe access.
#[derive(Debug)]
pub struct CleanupManager {
    /// S3 bucket handle for delete operations
    bucket: Bucket,
    
    /// List of currently active test files that need cleanup
    /// 
    /// Files are added when uploaded and removed when successfully cleaned up.
    /// This allows for emergency cleanup of all remaining files if needed.
    active_files: Arc<Mutex<Vec<String>>>,
}

impl CleanupManager {
    /// Creates a new cleanup manager
    /// 
    /// # Arguments
    /// 
    /// * `bucket` - S3 bucket handle for performing delete operations
    /// 
    /// # Returns
    /// 
    /// A new `CleanupManager` instance ready for use
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let cleanup = CleanupManager::new(bucket);
    /// ```
    pub fn new(bucket: Bucket) -> Self {
        Self {
            bucket,
            active_files: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    /// Registers a test file as active
    /// 
    /// Adds the file to the internal tracking list so it can be cleaned up
    /// later, even in emergency scenarios. This should be called immediately
    /// after a successful file upload.
    /// 
    /// # Arguments
    /// 
    /// * `file_key` - The S3 object key to track
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// cleanup.register_file("consistency-test-abc123").await;
    /// ```
    pub async fn register_file(&self, file_key: &str) {
        debug!("Registering test file for cleanup: {}", file_key);
        let mut active_files = self.active_files.lock().await;
        active_files.push(file_key.to_string());
    }
    
    /// Unregisters a test file from active tracking
    /// 
    /// Removes the file from the internal tracking list. This should be called
    /// after successful cleanup to prevent unnecessary emergency cleanup attempts.
    /// 
    /// # Arguments
    /// 
    /// * `file_key` - The S3 object key to unregister
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// cleanup.unregister_file("consistency-test-abc123").await;
    /// ```
    pub async fn unregister_file(&self, file_key: &str) {
        debug!("Unregistering test file from cleanup tracking: {}", file_key);
        let mut active_files = self.active_files.lock().await;
        active_files.retain(|f| f != file_key);
    }
    
    /// Cleans up a single test file with retry logic
    /// 
    /// Attempts to delete the specified file from S3 with automatic retries
    /// on failure. Uses exponential backoff between attempts to avoid
    /// overwhelming the S3 service.
    /// 
    /// # Arguments
    /// 
    /// * `file_key` - The S3 object key to delete
    /// 
    /// # Behavior
    /// 
    /// - Makes up to 3 attempts to delete the file
    /// - Waits 1 second between attempts  
    /// - Logs warnings for failed attempts
    /// - Logs errors if all attempts fail
    /// - Automatically unregisters the file if deletion succeeds
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// cleanup.cleanup_file("consistency-test-abc123").await;
    /// ```
    pub async fn cleanup_file(&self, file_key: &str) {
        debug!("Starting cleanup for test file: {}", file_key);
        
        // Try multiple times to ensure cleanup succeeds
        for attempt in 1..=3 {
            match self.bucket.delete_object(file_key).await {
                Ok(_) => {
                    debug!("Successfully cleaned up test file: {}", file_key);
                    self.unregister_file(file_key).await;
                    return;
                }
                Err(e) => {
                    if attempt == 3 {
                        error!(
                            "Failed to clean up test file {} after {} attempts: {}",
                            file_key, attempt, e
                        );
                    } else {
                        warn!(
                            "Cleanup attempt {} failed for {}: {}, retrying in 1s...",
                            attempt, file_key, e
                        );
                        sleep(Duration::from_millis(1000)).await;
                    }
                }
            }
        }
    }
    
    /// Performs emergency cleanup of all active test files
    /// 
    /// This method is designed to be called when the program is shutting down
    /// unexpectedly (e.g., due to Ctrl+C) to ensure no test files are left
    /// behind in the S3 bucket.
    /// 
    /// # Behavior
    /// 
    /// - Gets a snapshot of all currently active files
    /// - Attempts to clean up each file individually
    /// - Continues even if some cleanups fail
    /// - Clears the active file list when complete
    /// - Logs progress and completion status
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// // In a signal handler
    /// cleanup.emergency_cleanup().await;
    /// ```
    pub async fn emergency_cleanup(&self) {
        // Get a snapshot of active files to avoid holding the lock during cleanup
        let active_files = {
            let active_files_guard = self.active_files.lock().await;
            active_files_guard.clone()
        };
        
        if active_files.is_empty() {
            debug!("No active test files to clean up");
            return;
        }
        
        info!("Starting emergency cleanup of {} test files...", active_files.len());
        
        // Clean up each file individually
        for file_key in &active_files {
            // Use a simpler cleanup for emergency scenarios (no retries to speed up shutdown)
            match self.bucket.delete_object(file_key).await {
                Ok(_) => {
                    debug!("Emergency cleanup successful for: {}", file_key);
                }
                Err(e) => {
                    warn!("Emergency cleanup failed for {}: {}", file_key, e);
                }
            }
        }
        
        // Clear the active file list
        {
            let mut active_files_guard = self.active_files.lock().await;
            active_files_guard.clear();
        }
        
        info!("Emergency cleanup completed for {} files", active_files.len());
    }
    
    /// Gets the number of currently active test files
    /// 
    /// Returns the count of files currently tracked for cleanup.
    /// Useful for monitoring and reporting purposes.
    /// 
    /// # Returns
    /// 
    /// The number of active test files
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let count = cleanup.active_file_count().await;
    /// println!("Currently tracking {} test files", count);
    /// ```
    pub async fn active_file_count(&self) -> usize {
        let active_files = self.active_files.lock().await;
        active_files.len()
    }
    
    /// Gets a list of all currently active test files
    /// 
    /// Returns a snapshot of all files currently being tracked for cleanup.
    /// Useful for debugging and reporting purposes.
    /// 
    /// # Returns
    /// 
    /// A vector containing all active file keys
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let files = cleanup.get_active_files().await;
    /// for file in files {
    ///     println!("Active file: {}", file);
    /// }
    /// ```
    pub async fn get_active_files(&self) -> Vec<String> {
        let active_files = self.active_files.lock().await;
        active_files.clone()
    }
    
    /// Performs a final cleanup check
    /// 
    /// This method should be called at the end of testing to ensure
    /// all test files have been properly cleaned up. It will attempt
    /// to clean up any remaining files and log warnings if any are found.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// // At the end of testing
    /// cleanup.final_cleanup_check().await;
    /// ```
    pub async fn final_cleanup_check(&self) {
        let active_count = self.active_file_count().await;
        
        if active_count > 0 {
            warn!(
                "Found {} test files still active at end of testing, cleaning up...",
                active_count
            );
            self.emergency_cleanup().await;
        } else {
            debug!("All test files cleaned up successfully");
        }
    }
}

/// Sets up a cleanup signal handler for graceful shutdown
/// 
/// This function sets up a Ctrl+C signal handler that will trigger
/// emergency cleanup when the program is interrupted. This ensures
/// test files are cleaned up even if the program is terminated unexpectedly.
/// 
/// # Arguments
/// 
/// * `cleanup_manager` - The cleanup manager to use for emergency cleanup
/// 
/// # Behavior
/// 
/// - Spawns a background task to listen for Ctrl+C
/// - Triggers emergency cleanup when signal is received
/// - Exits the program after cleanup is complete
/// - Logs the cleanup process
/// 
/// # Examples
/// 
/// ```rust
/// let cleanup = Arc::new(CleanupManager::new(bucket));
/// setup_cleanup_handler(cleanup.clone());
/// ```
pub fn setup_cleanup_handler(cleanup_manager: Arc<CleanupManager>) {
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                warn!("Received interrupt signal (Ctrl+C), initiating cleanup...");
                cleanup_manager.emergency_cleanup().await;
                info!("Cleanup completed, exiting");
                std::process::exit(1);
            }
            Err(err) => {
                error!("Failed to listen for shutdown signal: {}", err);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use s3::{Bucket, Region, creds::Credentials};

    fn create_test_bucket() -> Bucket {
        let credentials = Credentials::default().unwrap();
        let region = Region::Custom {
            region: "test".to_string(),
            endpoint: "http://localhost:9000".to_string(),
        };
        Bucket::new("test-bucket", region, credentials).unwrap()
    }

    #[tokio::test]
    async fn test_register_and_unregister_file() {
        let bucket = create_test_bucket();
        let cleanup = CleanupManager::new(bucket);
        
        assert_eq!(cleanup.active_file_count().await, 0);
        
        cleanup.register_file("test-file-1").await;
        cleanup.register_file("test-file-2").await;
        
        assert_eq!(cleanup.active_file_count().await, 2);
        
        cleanup.unregister_file("test-file-1").await;
        
        assert_eq!(cleanup.active_file_count().await, 1);
        
        let active_files = cleanup.get_active_files().await;
        assert_eq!(active_files, vec!["test-file-2"]);
    }
    
    #[tokio::test]
    async fn test_get_active_files() {
        let bucket = create_test_bucket();
        let cleanup = CleanupManager::new(bucket);
        
        cleanup.register_file("file-a").await;
        cleanup.register_file("file-b").await;
        cleanup.register_file("file-c").await;
        
        let active_files = cleanup.get_active_files().await;
        assert_eq!(active_files.len(), 3);
        assert!(active_files.contains(&"file-a".to_string()));
        assert!(active_files.contains(&"file-b".to_string()));
        assert!(active_files.contains(&"file-c".to_string()));
    }
}