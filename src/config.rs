//! Configuration module for S3 consistency testing
//! 
//! This module handles loading and managing S3 configuration from TOML files.
//! It supports various S3-compatible storage providers including AWS S3, MinIO,
//! DigitalOcean Spaces, Cloudflare R2, and other S3-compatible services.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// S3 configuration structure
/// 
/// Contains all necessary connection details for S3-compatible storage services.
/// This configuration is typically loaded from a TOML file.
/// 
/// # Examples
/// 
/// ```toml
/// # AWS S3 configuration
/// endpoint = "https://s3.amazonaws.com"
/// region = "us-east-1"
/// bucket = "my-test-bucket"
/// access_key = "AKIAIOSFODNN7EXAMPLE"
/// secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
/// path_style = false
/// 
/// # MinIO configuration
/// endpoint = "http://localhost:9000"
/// region = "us-east-1"
/// bucket = "my-test-bucket"
/// access_key = "minioadmin"
/// secret_key = "minioadmin"
/// path_style = true
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct S3Config {
    /// The S3 endpoint URL
    /// 
    /// Examples:
    /// - AWS S3: "https://s3.amazonaws.com"
    /// - MinIO: "http://localhost:9000"
    /// - DigitalOcean Spaces: "https://nyc3.digitaloceanspaces.com"
    /// - Cloudflare R2: "https://account-id.r2.cloudflarestorage.com"
    pub endpoint: String,
    
    /// The S3 region
    /// 
    /// For AWS S3, use standard regions like "us-east-1", "eu-west-1", etc.
    /// For custom S3 implementations, this can be any string (often "us-east-1" works).
    pub region: String,
    
    /// The name of the S3 bucket to test against
    /// 
    /// This bucket must already exist and be accessible with the provided credentials.
    /// The test will create and delete temporary files in this bucket.
    pub bucket: String,
    
    /// S3 access key ID
    /// 
    /// The access key for authenticating with the S3 service.
    pub access_key: String,
    
    /// S3 secret access key
    /// 
    /// The secret key corresponding to the access key.
    pub secret_key: String,
    
    /// Whether to use path-style URLs
    /// 
    /// - `true` for path-style: `http://host/bucket/key`
    /// - `false` for virtual-hosted-style: `http://bucket.host/key`
    /// 
    /// Most AWS S3 configurations use `false`, while MinIO and some other
    /// S3-compatible services require `true`.
    pub path_style: Option<bool>,
}

impl S3Config {
    /// Validates the S3 configuration
    /// 
    /// Checks that all required fields are present and non-empty.
    /// 
    /// # Returns
    /// 
    /// - `Ok(())` if the configuration is valid
    /// - `Err(anyhow::Error)` if validation fails
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// let config = S3Config { /* ... */ };
    /// config.validate()?;
    /// ```
    pub fn validate(&self) -> Result<()> {
        if self.endpoint.trim().is_empty() {
            return Err(anyhow::anyhow!("S3 endpoint cannot be empty"));
        }
        
        if self.region.trim().is_empty() {
            return Err(anyhow::anyhow!("S3 region cannot be empty"));
        }
        
        if self.bucket.trim().is_empty() {
            return Err(anyhow::anyhow!("S3 bucket name cannot be empty"));
        }
        
        if self.access_key.trim().is_empty() {
            return Err(anyhow::anyhow!("S3 access key cannot be empty"));
        }
        
        if self.secret_key.trim().is_empty() {
            return Err(anyhow::anyhow!("S3 secret key cannot be empty"));
        }
        
        // Validate endpoint format
        if !self.endpoint.starts_with("http://") && !self.endpoint.starts_with("https://") {
            return Err(anyhow::anyhow!("S3 endpoint must start with http:// or https://"));
        }
        
        Ok(())
    }
    
    /// Determines if this is an AWS S3 configuration
    /// 
    /// Returns `true` if the endpoint appears to be an AWS S3 endpoint.
    /// This is used to determine whether to use AWS-specific region handling.
    /// 
    /// # Returns
    /// 
    /// `true` if the endpoint contains "amazonaws.com", `false` otherwise.
    pub fn is_aws_s3(&self) -> bool {
        self.endpoint.contains("amazonaws.com")
    }
    
    /// Gets the path style setting with a sensible default
    /// 
    /// Returns the explicit path_style setting if provided, otherwise
    /// uses a default based on the endpoint type:
    /// - AWS S3: defaults to `false` (virtual-hosted-style)
    /// - Other services: defaults to `true` (path-style)
    /// 
    /// # Returns
    /// 
    /// The path style setting to use
    pub fn get_path_style(&self) -> bool {
        self.path_style.unwrap_or(!self.is_aws_s3())
    }
}

/// Loads S3 configuration from a TOML file
/// 
/// Reads the specified file and parses it as TOML, deserializing into
/// an `S3Config` structure. The configuration is validated after loading.
/// 
/// # Arguments
/// 
/// * `path` - Path to the TOML configuration file
/// 
/// # Returns
/// 
/// - `Ok(S3Config)` if the configuration was loaded and validated successfully
/// - `Err(anyhow::Error)` if loading or validation fails
/// 
/// # Errors
/// 
/// This function will return an error if:
/// - The file cannot be read
/// - The file is not valid TOML
/// - The TOML doesn't match the expected S3Config structure
/// - The configuration fails validation
/// 
/// # Examples
/// 
/// ```rust
/// use std::path::PathBuf;
/// 
/// let config_path = PathBuf::from("config.toml");
/// let config = load_config(&config_path)?;
/// println!("Loaded config for bucket: {}", config.bucket);
/// ```
pub fn load_config(path: &PathBuf) -> Result<S3Config> {
    // Read the configuration file
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    
    // Parse as TOML
    let config: S3Config = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file as TOML: {}", path.display()))?;
    
    // Validate the configuration
    config.validate()
        .with_context(|| "Configuration validation failed")?;
    
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_s3_config_validation() {
        let valid_config = S3Config {
            endpoint: "https://s3.amazonaws.com".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            path_style: Some(false),
        };
        
        assert!(valid_config.validate().is_ok());
    }
    
    #[test]
    fn test_s3_config_validation_empty_fields() {
        let invalid_config = S3Config {
            endpoint: "".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            path_style: Some(false),
        };
        
        assert!(invalid_config.validate().is_err());
    }
    
    #[test]
    fn test_is_aws_s3() {
        let aws_config = S3Config {
            endpoint: "https://s3.amazonaws.com".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            path_style: None,
        };
        
        assert!(aws_config.is_aws_s3());
        
        let minio_config = S3Config {
            endpoint: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            path_style: None,
        };
        
        assert!(!minio_config.is_aws_s3());
    }
    
    #[test]
    fn test_get_path_style() {
        // Explicit setting should be used
        let explicit_config = S3Config {
            endpoint: "https://s3.amazonaws.com".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            path_style: Some(true),
        };
        
        assert!(explicit_config.get_path_style());
        
        // AWS S3 should default to false
        let aws_config = S3Config {
            endpoint: "https://s3.amazonaws.com".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            path_style: None,
        };
        
        assert!(!aws_config.get_path_style());
        
        // Non-AWS should default to true
        let minio_config = S3Config {
            endpoint: "http://localhost:9000".to_string(),
            region: "us-east-1".to_string(),
            bucket: "test-bucket".to_string(),
            access_key: "test-key".to_string(),
            secret_key: "test-secret".to_string(),
            path_style: None,
        };
        
        assert!(minio_config.get_path_style());
    }
}