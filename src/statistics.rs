//! Statistical analysis and reporting for S3 consistency test results
//! 
//! This module provides functions for calculating statistics from consistency test
//! results and formatting them for display. It handles percentile calculations,
//! success rate analysis, and comprehensive reporting.

use crate::types::{ConsistencyReport, ConsistencyStatistics, TestResult};

/// Calculates comprehensive statistics from test results
/// 
/// Analyzes a collection of test results to produce statistical measures
/// including success rates, timing percentiles, and distribution metrics.
/// Only successful tests with valid propagation times are included in timing statistics.
/// 
/// # Arguments
/// 
/// * `results` - Slice of test results to analyze
/// 
/// # Returns
/// 
/// A `ConsistencyStatistics` structure containing calculated metrics
/// 
/// # Examples
/// 
/// ```rust
/// let statistics = calculate_statistics(&test_results);
/// println!("Success rate: {:.1}%", statistics.success_rate);
/// ```
pub fn calculate_statistics(results: &[TestResult]) -> ConsistencyStatistics {
    if results.is_empty() {
        return ConsistencyStatistics {
            successful_tests: 0,
            failed_tests: 0,
            success_rate: 0.0,
            min_propagation_time_ms: None,
            max_propagation_time_ms: None,
            avg_propagation_time_ms: None,
            median_propagation_time_ms: None,
            percentile_95_ms: None,
            percentile_99_ms: None,
        };
    }
    
    // Separate successful results with valid propagation times
    let successful_results: Vec<_> = results
        .iter()
        .filter(|r| r.success && r.propagation_duration_ms.is_some())
        .collect();
    
    let successful_tests = successful_results.len();
    let failed_tests = results.len() - successful_tests;
    let success_rate = (successful_tests as f64 / results.len() as f64) * 100.0;
    
    // If no successful results, return basic statistics
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
    
    // Extract and sort propagation times for percentile calculations
    let mut durations: Vec<u64> = successful_results
        .iter()
        .map(|r| r.propagation_duration_ms.unwrap())
        .collect();
    durations.sort_unstable();
    
    // Calculate basic statistics
    let min_propagation_time_ms = durations.first().copied();
    let max_propagation_time_ms = durations.last().copied();
    let avg_propagation_time_ms = Some(
        durations.iter().sum::<u64>() as f64 / durations.len() as f64
    );
    
    // Calculate median
    let median_propagation_time_ms = calculate_median(&durations);
    
    // Calculate percentiles
    let percentile_95_ms = calculate_percentile(&durations, 95.0);
    let percentile_99_ms = calculate_percentile(&durations, 99.0);
    
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

/// Calculates the median value from a sorted vector of durations
/// 
/// For even-length vectors, returns the average of the two middle values.
/// For odd-length vectors, returns the exact middle value.
/// 
/// # Arguments
/// 
/// * `sorted_durations` - A sorted vector of duration values
/// 
/// # Returns
/// 
/// The median value, or `None` if the vector is empty
/// 
/// # Examples
/// 
/// ```rust
/// let durations = vec![100, 200, 300, 400, 500];
/// let median = calculate_median(&durations); // Some(300)
/// ```
fn calculate_median(sorted_durations: &[u64]) -> Option<u64> {
    if sorted_durations.is_empty() {
        return None;
    }
    
    let len = sorted_durations.len();
    if len % 2 == 0 {
        // Even number of elements - average the two middle values
        let mid1 = sorted_durations[len / 2 - 1];
        let mid2 = sorted_durations[len / 2];
        Some((mid1 + mid2) / 2)
    } else {
        // Odd number of elements - take the middle value
        Some(sorted_durations[len / 2])
    }
}

/// Calculates a specific percentile from a sorted vector of durations
/// 
/// Uses the nearest-rank method for percentile calculation.
/// For example, the 95th percentile is the value below which 95% of the data falls.
/// 
/// # Arguments
/// 
/// * `sorted_durations` - A sorted vector of duration values
/// * `percentile` - The percentile to calculate (0.0 to 100.0)
/// 
/// # Returns
/// 
/// The percentile value, or `None` if the vector is empty or percentile is invalid
/// 
/// # Examples
/// 
/// ```rust
/// let durations = vec![100, 200, 300, 400, 500];
/// let p95 = calculate_percentile(&durations, 95.0); // Some(500)
/// let p50 = calculate_percentile(&durations, 50.0); // Some(300)
/// ```
fn calculate_percentile(sorted_durations: &[u64], percentile: f64) -> Option<u64> {
    if sorted_durations.is_empty() || percentile < 0.0 || percentile > 100.0 {
        return None;
    }
    
    let len = sorted_durations.len();
    
    // Handle edge cases
    if percentile == 0.0 {
        return sorted_durations.first().copied();
    }
    if percentile == 100.0 {
        return sorted_durations.last().copied();
    }
    
    // Calculate the index using the nearest-rank method
    let rank = (percentile / 100.0) * (len as f64);
    let index = (rank.ceil() as usize).saturating_sub(1);
    
    // Ensure index is within bounds
    let index = index.min(len - 1);
    
    sorted_durations.get(index).copied()
}

/// Prints a comprehensive summary of test results to the console
/// 
/// Displays a formatted report including test configuration, success rates,
/// timing statistics, and individual test results. The output is designed
/// to be human-readable and suitable for console display.
/// 
/// # Arguments
/// 
/// * `report` - The consistency report to display
/// 
/// # Output Format
/// 
/// The summary includes:
/// - Test metadata (duration, endpoint, bucket, file count)
/// - Success/failure statistics with percentages
/// - Timing statistics (min, max, average, median, percentiles)
/// - Individual test results with status and timing
/// 
/// # Examples
/// 
/// ```rust
/// print_summary(&consistency_report);
/// ```
pub fn print_summary(report: &ConsistencyReport) {
    println!("\n{}", "=".repeat(50));
    println!("           S3 CONSISTENCY TEST SUMMARY");
    println!("{}", "=".repeat(50));
    
    // Test metadata
    println!("Test Duration: {}ms", report.total_duration_ms);
    println!("S3 Endpoint: {}", report.config.endpoint);
    println!("Bucket: {}", report.config.bucket);
    println!("Files Tested: {}", report.test_parameters.test_count);
    println!("File Size: {} bytes", report.test_parameters.file_size);
    println!("Max Wait Time: {}s", report.test_parameters.max_wait_seconds);
    println!("Check Interval: {}ms", report.test_parameters.check_interval_ms);
    
    println!("\n{}", "-".repeat(30));
    println!("RESULTS OVERVIEW");
    println!("{}", "-".repeat(30));
    
    let stats = &report.statistics;
    let total_tests = stats.successful_tests + stats.failed_tests;
    
    println!("✅ Successful Tests: {} ({:.1}%)", 
             stats.successful_tests, stats.success_rate);
    println!("❌ Failed Tests: {} ({:.1}%)", 
             stats.failed_tests, 
             100.0 - stats.success_rate);
    println!("📊 Total Tests: {}", total_tests);
    
    // Only show timing statistics if we have successful tests
    if stats.successful_tests > 0 {
        println!("\n{}", "-".repeat(30));
        println!("PROPAGATION TIMING ANALYSIS");
        println!("{}", "-".repeat(30));
        
        if let Some(min) = stats.min_propagation_time_ms {
            println!("⚡ Fastest: {}ms", min);
        }
        
        if let Some(max) = stats.max_propagation_time_ms {
            println!("🐌 Slowest: {}ms", max);
        }
        
        if let Some(avg) = stats.avg_propagation_time_ms {
            println!("📊 Average: {:.1}ms", avg);
        }
        
        if let Some(median) = stats.median_propagation_time_ms {
            println!("📈 Median: {}ms", median);
        }
        
        // Percentiles section
        println!("\n📋 Percentiles:");
        if let Some(p95) = stats.percentile_95_ms {
            println!("   95th: {}ms (95% of tests completed within this time)", p95);
        }
        if let Some(p99) = stats.percentile_99_ms {
            println!("   99th: {}ms (99% of tests completed within this time)", p99);
        }
        
        // Distribution analysis
        print_distribution_analysis(stats);
    }
    
    // Individual test results
    println!("\n{}", "-".repeat(30));
    println!("INDIVIDUAL TEST RESULTS");
    println!("{}", "-".repeat(30));
    
    for (i, result) in report.results.iter().enumerate() {
        print!("Test {:2}: ", i + 1);
        
        if result.success {
            if let Some(duration) = result.propagation_duration_ms {
                println!("✅ SUCCESS - {}ms ({} attempts)", 
                         duration, result.total_attempts);
            } else {
                println!("✅ SUCCESS - immediate");
            }
        } else {
            let error_msg = result.error_details
                .as_deref()
                .unwrap_or("Unknown error");
            println!("❌ FAILED - {}", error_msg);
        }
    }
    
    println!("\n{}", "=".repeat(50));
    println!("Report saved to: consistency-report-{}.json", 
             report.test_start_time.format("%Y%m%d-%H%M%S"));
    println!("{}", "=".repeat(50));
}

/// Provides additional analysis of the timing distribution
/// 
/// Analyzes the consistency statistics to provide insights about
/// the distribution of propagation times and potential performance characteristics.
/// 
/// # Arguments
/// 
/// * `stats` - The consistency statistics to analyze
fn print_distribution_analysis(stats: &ConsistencyStatistics) {
    println!("\n🔍 Distribution Analysis:");
    
    if let (Some(min), Some(max), Some(avg)) = (
        stats.min_propagation_time_ms,
        stats.max_propagation_time_ms,
        stats.avg_propagation_time_ms,
    ) {
        let range = max - min;
        let variance_indicator = if range > (avg * 2.0) as u64 {
            "High variance - inconsistent propagation times"
        } else if range > avg as u64 {
            "Moderate variance - some variation in propagation times"
        } else {
            "Low variance - consistent propagation times"
        };
        
        println!("   Range: {}ms ({}ms to {}ms)", range, min, max);
        println!("   Consistency: {}", variance_indicator);
        
        // Performance assessment
        if avg < 100.0 {
            println!("   Performance: ⚡ Excellent (< 100ms average)");
        } else if avg < 500.0 {
            println!("   Performance: ✅ Good (< 500ms average)");
        } else if avg < 2000.0 {
            println!("   Performance: ⚠️  Moderate (< 2s average)");
        } else {
            println!("   Performance: ❌ Slow (> 2s average)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::config::S3Config;
    use crate::types::TestParameters;

    fn create_test_result(success: bool, duration_ms: Option<u64>) -> TestResult {
        let now = Utc::now();
        if success {
            TestResult::success(
                "test-key".to_string(),
                now,
                now,
                1,
            )
        } else {
            TestResult::failure(
                "test-key".to_string(),
                now,
                "Test error".to_string(),
            )
        }
    }

    #[test]
    fn test_calculate_median_odd_length() {
        let durations = vec![100, 200, 300, 400, 500];
        assert_eq!(calculate_median(&durations), Some(300));
    }

    #[test]
    fn test_calculate_median_even_length() {
        let durations = vec![100, 200, 300, 400];
        assert_eq!(calculate_median(&durations), Some(250));
    }

    #[test]
    fn test_calculate_median_empty() {
        let durations = vec![];
        assert_eq!(calculate_median(&durations), None);
    }

    #[test]
    fn test_calculate_percentile() {
        let durations = vec![100, 200, 300, 400, 500];
        
        assert_eq!(calculate_percentile(&durations, 0.0), Some(100));
        assert_eq!(calculate_percentile(&durations, 50.0), Some(300));
        assert_eq!(calculate_percentile(&durations, 100.0), Some(500));
        
        // Test edge cases
        assert_eq!(calculate_percentile(&vec![], 50.0), None);
        assert_eq!(calculate_percentile(&durations, -1.0), None);
        assert_eq!(calculate_percentile(&durations, 101.0), None);
    }

    #[test]
    fn test_calculate_statistics_empty() {
        let results = vec![];
        let stats = calculate_statistics(&results);
        
        assert_eq!(stats.successful_tests, 0);
        assert_eq!(stats.failed_tests, 0);
        assert_eq!(stats.success_rate, 0.0);
        assert_eq!(stats.min_propagation_time_ms, None);
    }

    #[test]
    fn test_calculate_statistics_mixed_results() {
        let results = vec![
            create_test_result(true, Some(100)),
            create_test_result(true, Some(200)),
            create_test_result(false, None),
            create_test_result(true, Some(300)),
        ];
        
        let stats = calculate_statistics(&results);
        
        assert_eq!(stats.successful_tests, 3);
        assert_eq!(stats.failed_tests, 1);
        assert_eq!(stats.success_rate, 75.0);
        assert_eq!(stats.min_propagation_time_ms, Some(100));
        assert_eq!(stats.max_propagation_time_ms, Some(300));
    }
}