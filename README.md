# S3 Consistency Test Tool

An extensive Rust tool for measuring eventual consistency propagation times in S3-compatible storage systems. This tool uploads test files and measures exactly how long it takes for them to become consistently readable across the storage infrastructure.

## Features

🌐 **Universal S3 Support**
- Amazon S3
- MinIO
- DigitalOcean Spaces
- Cloudflare R2
- Google Cloud Storage (S3 API)
- Any S3-compatible storage service

⏱️ **Precise Measurements**
- Millisecond-accurate timing
- Detailed propagation time statistics
- Comprehensive percentile analysis (95th, 99th)
- Individual test result tracking

📊 **Rich Analytics**
- Min, max, average, median timing
- Success/failure rate analysis  
- Distribution variance assessment
- Performance categorization

🛡️ **Robust & Reliable**
- Automatic test file cleanup with retry logic
- Emergency cleanup on program interruption (Ctrl+C)
- Structured error handling with helpful context
- Thread-safe operations

📁 **Professional Output**
- Beautiful formatted console reports
- Detailed JSON reports for analysis
- Real-time progress monitoring
- Configurable logging levels

## Installation

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs/))
- Access to an S3-compatible storage service

### Build from Source

```bash
git clone https://github.com/your-username/S3-Consistancy-Test.git
cd S3-Consistancy-Test
cargo build --release
```

The binary will be available at `target/release/S3-Consistancy-Test` (or `.exe` on Windows).

## Configuration

Create a `config.toml` file with your S3 connection details:

```toml
# S3 Configuration
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
bucket = "your-test-bucket"
access_key = "your-access-key"
secret_key = "your-secret-key"
path_style = false
```

### Configuration Options

| Field | Description | Required |
|-------|-------------|----------|
| `endpoint` | S3 service endpoint URL | ✅ |
| `region` | S3 region (e.g., "us-east-1") | ✅ |
| `bucket` | Target bucket name (must exist) | ✅ |
| `access_key` | S3 access key ID | ✅ |
| `secret_key` | S3 secret access key | ✅ |
| `path_style` | Use path-style URLs (true for MinIO) | ❌ |

### Example Configurations

#### AWS S3
```toml
endpoint = "https://s3.amazonaws.com"
region = "us-east-1"
bucket = "my-test-bucket"
access_key = "AKIAIOSFODNN7EXAMPLE"
secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
path_style = false
```

#### MinIO
```toml
endpoint = "http://localhost:9000"
region = "us-east-1"
bucket = "test-bucket"
access_key = "minioadmin"
secret_key = "minioadmin"
path_style = true
```

#### DigitalOcean Spaces
```toml
endpoint = "https://nyc3.digitaloceanspaces.com"
region = "nyc3"
bucket = "my-space"
access_key = "your-spaces-key"
secret_key = "your-spaces-secret"
path_style = false
```

#### Cloudflare R2
```toml
endpoint = "https://account-id.r2.cloudflarestorage.com"
region = "auto"
bucket = "my-bucket"
access_key = "your-r2-token"
secret_key = "your-r2-secret"
path_style = false
```

## Usage

### Basic Usage

```bash
# Run with default settings (10 files, 1KB each)
cargo run -- --config config.toml
```

### Advanced Usage

```bash
# Custom test parameters
cargo run -- --config config.toml \
  --test-count 50 \
  --file-size 4096 \
  --max-wait 120 \
  --interval 200 \
  --verbose
```

### Command Line Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--config` | `-c` | Path to configuration file | *required* |
| `--test-count` | `-t` | Number of files to test | 10 |
| `--file-size` | `-f` | File size in bytes | 1024 |
| `--max-wait` | `-m` | Max wait time (seconds) | 300 |
| `--interval` | `-i` | Check interval (milliseconds) | 100 |
| `--verbose` | `-v` | Enable debug logging | false |

### Getting Help

```bash
cargo run -- --help
```

## Sample Output

```
==================================================
           S3 CONSISTENCY TEST SUMMARY
==================================================
Test Duration: 45123ms
S3 Endpoint: https://s3.amazonaws.com
Bucket: my-test-bucket
Files Tested: 20
File Size: 2048 bytes
Max Wait Time: 300s
Check Interval: 100ms

------------------------------
RESULTS OVERVIEW
------------------------------
✅ Successful Tests: 19 (95.0%)
❌ Failed Tests: 1 (5.0%)
📊 Total Tests: 20

------------------------------
PROPAGATION TIMING ANALYSIS
------------------------------
⚡ Fastest: 145ms
🐌 Slowest: 2341ms
📊 Average: 892.3ms
📈 Median: 756ms

📋 Percentiles:
   95th: 1987ms (95% of tests completed within this time)
   99th: 2341ms (99% of tests completed within this time)

🔍 Distribution Analysis:
   Range: 2196ms (145ms to 2341ms)
   Consistency: Moderate variance - some variation in propagation times
   Performance: ✅ Good (< 500ms average)

------------------------------
INDIVIDUAL TEST RESULTS
------------------------------
Test  1: ✅ SUCCESS - 234ms (3 attempts)
Test  2: ✅ SUCCESS - 456ms (5 attempts)
Test  3: ✅ SUCCESS - 123ms (2 attempts)
...
Test 20: ❌ FAILED - Consistency test timed out

==================================================
Report saved to: consistency-report-20250123-143022.json
==================================================
```

## JSON Report Format

The tool generates detailed JSON reports with complete test data:

```json
{
  "test_start_time": "2025-01-23T14:30:22.123Z",
  "test_end_time": "2025-01-23T14:31:07.456Z",
  "total_duration_ms": 45333,
  "config": {
    "endpoint": "https://s3.amazonaws.com",
    "bucket": "my-test-bucket",
    ...
  },
  "test_parameters": {
    "test_count": 20,
    "file_size": 2048,
    "max_wait_seconds": 300,
    "check_interval_ms": 100
  },
  "results": [
    {
      "file_key": "consistency-test-550e8400-e29b-41d4-a716-446655440000",
      "upload_time": "2025-01-23T14:30:22.234Z",
      "first_read_success_time": "2025-01-23T14:30:22.468Z",
      "propagation_duration_ms": 234,
      "total_attempts": 3,
      "success": true,
      "error_details": null
    }
    ...
  ],
  "statistics": {
    "successful_tests": 19,
    "failed_tests": 1,
    "success_rate": 95.0,
    "min_propagation_time_ms": 145,
    "max_propagation_time_ms": 2341,
    "avg_propagation_time_ms": 892.3,
    "median_propagation_time_ms": 756,
    "percentile_95_ms": 1987,
    "percentile_99_ms": 2341
  }
}
```

## Architecture

The tool is built with a clean, modular architecture:

```
src/
├── main.rs          # Application entry point and orchestration
├── config.rs        # Configuration loading and validation
├── types.rs         # Data structures and CLI definitions
├── tester.rs        # Core S3 testing logic
├── cleanup.rs       # File cleanup and signal handling
└── statistics.rs    # Statistical analysis and reporting
```

### Key Components

- **Configuration Management**: Robust TOML-based configuration with validation
- **S3 Testing Engine**: Handles uploads, consistency checks, and timing
- **Cleanup Manager**: Ensures no test files are left behind
- **Statistics Calculator**: Comprehensive analysis with percentiles
- **Report Generator**: Beautiful console and JSON output

## How It Works

1. **Upload Phase**: Creates unique test files with random data and uploads them to S3
2. **Polling Phase**: Continuously attempts to read each file until it becomes available
3. **Timing Measurement**: Records precise timestamps for propagation duration calculation
4. **Cleanup Phase**: Automatically removes all test files with retry logic
5. **Analysis Phase**: Calculates comprehensive statistics and generates reports

## Use Cases

### Performance Testing
- Measure S3 service consistency behavior
- Compare different S3 providers
- Validate SLA compliance

### Infrastructure Monitoring
- Monitor consistency degradation over time
- Alert on unusual propagation delays
- Track performance across regions

### Research & Analysis
- Study eventual consistency patterns
- Analyze the impact of file size on propagation
- Generate data for academic research

## Troubleshooting

### Common Issues

**Connection Errors**
```
Failed to initialize S3 tester: Failed to create S3 bucket handle
```
- Verify endpoint URL is correct
- Check network connectivity
- Ensure credentials have proper permissions

**Permission Errors**
```
Upload failed: Access Denied
```
- Verify access key and secret key
- Ensure bucket exists and is accessible
- Check IAM permissions for PUT/GET/DELETE operations

**Timeout Issues**
```
Consistency test timed out after 300 attempts
```
- Increase `--max-wait` parameter
- Check if the S3 service is experiencing issues
- Verify bucket is in the correct region

### Debug Mode

Enable verbose logging for detailed information:

```bash
cargo run -- --config config.toml --verbose
```

This shows:
- Individual upload operations
- Each read attempt with timing
- Cleanup operations and retries
- Detailed error messages

## Performance Considerations

### Optimal Settings

- **Test Count**: 10-50 files for good statistical accuracy
- **File Size**: 1KB-10KB for reasonable upload/download times
- **Check Interval**: 50-200ms balances precision vs. load
- **Max Wait**: 120-300s depending on expected consistency times

### Resource Usage

- **Memory**: Minimal (< 10MB typical)
- **Network**: Proportional to file size × test count × attempts
- **Storage**: Temporary (files are cleaned up automatically)

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Development Setup

```bash
git clone https://github.com/your-username/S3-Consistancy-Test.git
cd S3-Consistancy-Test
cargo test
cargo clippy
cargo fmt
```

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests (requires S3 credentials)
cargo test --test integration
```

## Security

- **Credentials**: Never commit credentials to version control
- **Permissions**: Use minimal required S3 permissions (GET, PUT, DELETE on test bucket)
- **Cleanup**: Tool automatically removes all test data
- **Reports**: Be cautious sharing JSON reports as they may contain sensitive configuration

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with the [rust-s3](https://crates.io/crates/rust-s3) crate
- Uses [tokio](https://tokio.rs/) for async operations
- CLI powered by [clap](https://docs.rs/clap/latest/clap/)
- Statistics calculations inspired by industry best practices

---

**Need help?** Open an issue on GitHub or check the troubleshooting section above.
