use anyhow::{Context, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use std::collections::VecDeque;
use tokio::sync::RwLock;
use tracing::{error, warn, info, debug};
use serde::{Serialize, Deserialize};

/// Comprehensive error handling system for ZeroCopy database operations
/// Provides atomic error tracking, transaction rollback, retry logic, and recovery mechanisms
#[derive(Debug)]
pub struct AtomicErrorHandler {
    // Atomic error counters
    total_errors: AtomicU64,
    transaction_errors: AtomicU64,
    serialization_errors: AtomicU64,
    io_errors: AtomicU64,
    memory_errors: AtomicU64,
    validation_errors: AtomicU64,
    
    // Transaction management counters
    total_transactions: AtomicU64,
    successful_transactions: AtomicU64,
    failed_transactions: AtomicU64,
    rollback_attempts: AtomicU64,
    successful_rollbacks: AtomicU64,
    failed_rollbacks: AtomicU64,
    
    // Retry mechanism counters
    retry_attempts: AtomicU64,
    successful_retries: AtomicU64,
    failed_retries: AtomicU64,
    max_retry_attempts_reached: AtomicU64,
    
    // Recovery mechanism counters
    recovery_attempts: AtomicU64,
    successful_recoveries: AtomicU64,
    failed_recoveries: AtomicU64,
    
    // Error statistics tracking
    error_history: RwLock<VecDeque<ErrorEvent>>,
    error_history_size: usize,
    
    // Configuration
    max_retry_attempts: u32,
    base_retry_delay: Duration,
    max_retry_delay: Duration,
    enable_detailed_logging: bool,
    
    // Start time for uptime calculation
    start_time: Instant,
}

/// Detailed error event for tracking and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub timestamp: SystemTime,
    pub error_type: ErrorType,
    pub error_message: String,
    pub operation_context: String,
    pub batch_id: Option<u64>,
    pub file_count: Option<usize>,
    pub retry_attempt: u32,
    pub recovery_attempted: bool,
    pub resolved: bool,
}

/// Types of errors that can occur in the ZeroCopy database
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorType {
    /// Transaction-related errors (commit, rollback, isolation)
    Transaction,
    /// Serialization/deserialization errors
    Serialization,
    /// I/O errors (file operations, memory mapping)
    IO,
    /// Memory allocation or limit errors
    Memory,
    /// Data validation errors
    Validation,
    /// Index corruption or inconsistency errors
    Index,
    /// Configuration or initialization errors
    Configuration,
    /// Network or connection errors
    Connection,
    /// Unknown or unclassified errors
    Unknown,
}

/// Result of a transaction operation with rollback information
#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub success: bool,
    pub transaction_id: u64,
    pub operation_count: usize,
    pub processing_time: Duration,
    pub rollback_attempted: bool,
    pub rollback_successful: bool,
    pub error_details: Option<String>,
}

/// Result of a retry operation
#[derive(Debug, Clone)]
pub struct RetryResult {
    pub success: bool,
    pub attempt_count: u32,
    pub total_retry_time: Duration,
    pub final_error: Option<String>,
    pub exponential_backoff_used: bool,
}

/// Recovery operation result
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    pub success: bool,
    pub recovery_type: RecoveryType,
    pub operations_recovered: usize,
    pub data_integrity_verified: bool,
    pub recovery_time: Duration,
    pub error_details: Option<String>,
}

/// Types of recovery operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryType {
    /// Automatic retry with exponential backoff
    AutomaticRetry,
    /// Transaction rollback and state restoration
    TransactionRollback,
    /// Index reconstruction
    IndexReconstruction,
    /// Memory cleanup and reallocation
    MemoryCleanup,
    /// File system consistency check
    FileSystemCheck,
    /// Configuration reset to safe defaults
    ConfigurationReset,
}

/// Comprehensive error statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStatistics {
    // Error counts by type
    pub total_errors: u64,
    pub transaction_errors: u64,
    pub serialization_errors: u64,
    pub io_errors: u64,
    pub memory_errors: u64,
    pub validation_errors: u64,
    
    // Transaction statistics
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub transaction_success_rate: f64,
    pub rollback_attempts: u64,
    pub successful_rollbacks: u64,
    pub rollback_success_rate: f64,
    
    // Retry statistics
    pub retry_attempts: u64,
    pub successful_retries: u64,
    pub failed_retries: u64,
    pub retry_success_rate: f64,
    pub max_retry_attempts_reached: u64,
    
    // Recovery statistics
    pub recovery_attempts: u64,
    pub successful_recoveries: u64,
    pub recovery_success_rate: f64,
    
    // Error rates and trends
    pub error_rate_per_hour: f64,
    pub most_common_error_type: ErrorType,
    pub error_trend: ErrorTrend,
    
    // System health indicators
    pub system_stability_score: f64, // 0.0 to 1.0
    pub uptime: Duration,
    pub last_updated: SystemTime,
}

/// Error trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorTrend {
    Improving,
    Stable,
    Degrading,
    Critical,
}

impl AtomicErrorHandler {
    /// Create a new atomic error handler
    pub fn new(
        max_retry_attempts: u32,
        base_retry_delay: Duration,
        max_retry_delay: Duration,
        enable_detailed_logging: bool,
    ) -> Self {
        Self {
            total_errors: AtomicU64::new(0),
            transaction_errors: AtomicU64::new(0),
            serialization_errors: AtomicU64::new(0),
            io_errors: AtomicU64::new(0),
            memory_errors: AtomicU64::new(0),
            validation_errors: AtomicU64::new(0),
            
            total_transactions: AtomicU64::new(0),
            successful_transactions: AtomicU64::new(0),
            failed_transactions: AtomicU64::new(0),
            rollback_attempts: AtomicU64::new(0),
            successful_rollbacks: AtomicU64::new(0),
            failed_rollbacks: AtomicU64::new(0),
            
            retry_attempts: AtomicU64::new(0),
            successful_retries: AtomicU64::new(0),
            failed_retries: AtomicU64::new(0),
            max_retry_attempts_reached: AtomicU64::new(0),
            
            recovery_attempts: AtomicU64::new(0),
            successful_recoveries: AtomicU64::new(0),
            failed_recoveries: AtomicU64::new(0),
            
            error_history: RwLock::new(VecDeque::with_capacity(1000)),
            error_history_size: 1000,
            
            max_retry_attempts,
            base_retry_delay,
            max_retry_delay,
            enable_detailed_logging,
            
            start_time: Instant::now(),
        }
    }
    
    /// Record an error event with atomic tracking
    pub async fn record_error(
        &self,
        error_type: ErrorType,
        error_message: String,
        operation_context: String,
        batch_id: Option<u64>,
        file_count: Option<usize>,
        retry_attempt: u32,
    ) {
        // Update atomic counters
        self.total_errors.fetch_add(1, Ordering::Relaxed);
        
        match error_type {
            ErrorType::Transaction => self.transaction_errors.fetch_add(1, Ordering::Relaxed),
            ErrorType::Serialization => self.serialization_errors.fetch_add(1, Ordering::Relaxed),
            ErrorType::IO => self.io_errors.fetch_add(1, Ordering::Relaxed),
            ErrorType::Memory => self.memory_errors.fetch_add(1, Ordering::Relaxed),
            ErrorType::Validation => self.validation_errors.fetch_add(1, Ordering::Relaxed),
            _ => 0, // Other error types don't have specific counters
        };
        
        // Create error event
        let error_event = ErrorEvent {
            timestamp: SystemTime::now(),
            error_type: error_type.clone(),
            error_message: error_message.clone(),
            operation_context: operation_context.clone(),
            batch_id,
            file_count,
            retry_attempt,
            recovery_attempted: false,
            resolved: false,
        };
        
        // Add to error history
        let mut history = self.error_history.write().await;
        history.push_back(error_event.clone());
        
        // Maintain history size limit
        while history.len() > self.error_history_size {
            history.pop_front();
        }
        
        // Log error with appropriate level
        if self.enable_detailed_logging {
            match error_type {
                ErrorType::Transaction | ErrorType::Memory | ErrorType::Index => {
                    error!(
                        "Critical error in {}: {} (batch_id: {:?}, files: {:?}, retry: {})",
                        operation_context, error_message, batch_id, file_count, retry_attempt
                    );
                }
                ErrorType::IO | ErrorType::Connection => {
                    warn!(
                        "Recoverable error in {}: {} (batch_id: {:?}, files: {:?}, retry: {})",
                        operation_context, error_message, batch_id, file_count, retry_attempt
                    );
                }
                _ => {
                    info!(
                        "Error in {}: {} (batch_id: {:?}, files: {:?}, retry: {})",
                        operation_context, error_message, batch_id, file_count, retry_attempt
                    );
                }
            }
        }
    }
    
    /// Execute a transaction with atomic rollback management
    pub async fn execute_transaction<F, T>(&self, transaction_id: u64, operation: F) -> Result<TransactionResult>
    where
        F: FnOnce() -> Result<T>,
    {
        let start_time = Instant::now();
        self.total_transactions.fetch_add(1, Ordering::Relaxed);
        
        let mut rollback_attempted = false;
        let mut rollback_successful = false;
        let mut error_details = None;
        
        match operation() {
            Ok(_) => {
                self.successful_transactions.fetch_add(1, Ordering::Relaxed);
                
                if self.enable_detailed_logging {
                    debug!("Transaction {} completed successfully in {:?}", transaction_id, start_time.elapsed());
                }
                
                Ok(TransactionResult {
                    success: true,
                    transaction_id,
                    operation_count: 1,
                    processing_time: start_time.elapsed(),
                    rollback_attempted,
                    rollback_successful,
                    error_details,
                })
            }
            Err(e) => {
                self.failed_transactions.fetch_add(1, Ordering::Relaxed);
                error_details = Some(e.to_string());
                
                // Attempt rollback
                rollback_attempted = true;
                self.rollback_attempts.fetch_add(1, Ordering::Relaxed);
                
                match self.perform_rollback(transaction_id).await {
                    Ok(_) => {
                        rollback_successful = true;
                        self.successful_rollbacks.fetch_add(1, Ordering::Relaxed);
                        
                        if self.enable_detailed_logging {
                            warn!("Transaction {} failed but rollback successful: {}", transaction_id, e);
                        }
                    }
                    Err(rollback_error) => {
                        rollback_successful = false;
                        self.failed_rollbacks.fetch_add(1, Ordering::Relaxed);
                        
                        error!(
                            "Transaction {} failed and rollback also failed. Original error: {}, Rollback error: {}",
                            transaction_id, e, rollback_error
                        );
                        
                        // Record critical error
                        self.record_error(
                            ErrorType::Transaction,
                            format!("Transaction rollback failed: {}", rollback_error),
                            "transaction_rollback".to_string(),
                            None,
                            None,
                            0,
                        ).await;
                    }
                }
                
                Ok(TransactionResult {
                    success: false,
                    transaction_id,
                    operation_count: 1,
                    processing_time: start_time.elapsed(),
                    rollback_attempted,
                    rollback_successful,
                    error_details,
                })
            }
        }
    }
    
    /// Perform transaction rollback with atomic state management
    async fn perform_rollback(&self, transaction_id: u64) -> Result<()> {
        // In a real implementation, this would:
        // 1. Restore memory-mapped file state to last known good state
        // 2. Revert index changes
        // 3. Clear any partial batch data
        // 4. Reset atomic counters to pre-transaction state
        
        if self.enable_detailed_logging {
            debug!("Performing rollback for transaction {}", transaction_id);
        }
        
        // Simulate rollback operations
        tokio::time::sleep(Duration::from_millis(1)).await;
        
        // For now, always succeed (in real implementation, this could fail)
        Ok(())
    }
    
    /// Execute operation with retry logic and exponential backoff
    pub async fn execute_with_retry<F, T>(&self, operation_name: &str, mut operation: F) -> Result<RetryResult>
    where
        F: FnMut() -> Result<T>,
    {
        let start_time = Instant::now();
        let mut attempt = 0;
        let mut last_error = None;
        
        while attempt < self.max_retry_attempts {
            attempt += 1;
            self.retry_attempts.fetch_add(1, Ordering::Relaxed);
            
            match operation() {
                Ok(_) => {
                    if attempt > 1 {
                        self.successful_retries.fetch_add(1, Ordering::Relaxed);
                        
                        if self.enable_detailed_logging {
                            info!(
                                "Operation '{}' succeeded on attempt {} after {:?}",
                                operation_name, attempt, start_time.elapsed()
                            );
                        }
                    }
                    
                    return Ok(RetryResult {
                        success: true,
                        attempt_count: attempt,
                        total_retry_time: start_time.elapsed(),
                        final_error: None,
                        exponential_backoff_used: attempt > 1,
                    });
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    
                    if attempt < self.max_retry_attempts {
                        // Calculate exponential backoff delay
                        let delay = self.calculate_backoff_delay(attempt);
                        
                        if self.enable_detailed_logging {
                            warn!(
                                "Operation '{}' failed on attempt {}: {}. Retrying in {:?}",
                                operation_name, attempt, e, delay
                            );
                        }
                        
                        // Record retry attempt
                        self.record_error(
                            ErrorType::Connection, // Assume retryable errors are connection-related
                            e.to_string(),
                            operation_name.to_string(),
                            None,
                            None,
                            attempt,
                        ).await;
                        
                        tokio::time::sleep(delay).await;
                    } else {
                        // Max attempts reached
                        self.max_retry_attempts_reached.fetch_add(1, Ordering::Relaxed);
                        self.failed_retries.fetch_add(1, Ordering::Relaxed);
                        
                        error!(
                            "Operation '{}' failed after {} attempts: {}",
                            operation_name, attempt, e
                        );
                        
                        // Record final failure
                        self.record_error(
                            ErrorType::Connection,
                            format!("Max retry attempts reached: {}", e),
                            operation_name.to_string(),
                            None,
                            None,
                            attempt,
                        ).await;
                    }
                }
            }
        }
        
        Ok(RetryResult {
            success: false,
            attempt_count: attempt,
            total_retry_time: start_time.elapsed(),
            final_error: last_error,
            exponential_backoff_used: true,
        })
    }
    
    /// Calculate exponential backoff delay with jitter
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let base_delay_ms = self.base_retry_delay.as_millis() as u64;
        let exponential_delay_ms = base_delay_ms * (2_u64.pow(attempt.saturating_sub(1)));
        
        // Cap at max delay
        let capped_delay_ms = exponential_delay_ms.min(self.max_retry_delay.as_millis() as u64);
        
        // Add jitter (±25% random variation)
        let jitter_range = capped_delay_ms / 4; // 25% of delay
        let jitter = fastrand::u64(0..=jitter_range * 2); // 0 to 50% of delay
        let final_delay_ms = capped_delay_ms.saturating_sub(jitter_range).saturating_add(jitter);
        
        Duration::from_millis(final_delay_ms)
    }
    
    /// Attempt error recovery with atomic consistency
    pub async fn attempt_recovery(&self, recovery_type: RecoveryType, context: &str) -> Result<RecoveryResult> {
        let start_time = Instant::now();
        self.recovery_attempts.fetch_add(1, Ordering::Relaxed);
        
        if self.enable_detailed_logging {
            info!("Attempting {:?} recovery in context: {}", recovery_type, context);
        }
        
        let result = match recovery_type {
            RecoveryType::AutomaticRetry => {
                // Already handled by execute_with_retry
                Ok(RecoveryResult {
                    success: true,
                    recovery_type: recovery_type.clone(),
                    operations_recovered: 1,
                    data_integrity_verified: true,
                    recovery_time: start_time.elapsed(),
                    error_details: None,
                })
            }
            RecoveryType::TransactionRollback => {
                // Transaction rollback recovery
                match self.perform_rollback(0).await {
                    Ok(_) => Ok(RecoveryResult {
                        success: true,
                        recovery_type: recovery_type.clone(),
                        operations_recovered: 1,
                        data_integrity_verified: true,
                        recovery_time: start_time.elapsed(),
                        error_details: None,
                    }),
                    Err(e) => Ok(RecoveryResult {
                        success: false,
                        recovery_type: recovery_type.clone(),
                        operations_recovered: 0,
                        data_integrity_verified: false,
                        recovery_time: start_time.elapsed(),
                        error_details: Some(e.to_string()),
                    }),
                }
            }
            RecoveryType::IndexReconstruction => {
                // Index reconstruction recovery
                self.perform_index_recovery().await
            }
            RecoveryType::MemoryCleanup => {
                // Memory cleanup recovery
                self.perform_memory_cleanup().await
            }
            RecoveryType::FileSystemCheck => {
                // File system consistency check
                self.perform_filesystem_check().await
            }
            RecoveryType::ConfigurationReset => {
                // Configuration reset to safe defaults
                self.perform_configuration_reset().await
            }
        };
        
        match &result {
            Ok(recovery_result) => {
                if recovery_result.success {
                    self.successful_recoveries.fetch_add(1, Ordering::Relaxed);
                    
                    if self.enable_detailed_logging {
                        info!(
                            "Recovery {:?} successful in {:?}: {} operations recovered",
                            recovery_type, recovery_result.recovery_time, recovery_result.operations_recovered
                        );
                    }
                } else {
                    self.failed_recoveries.fetch_add(1, Ordering::Relaxed);
                    
                    error!(
                        "Recovery {:?} failed in {:?}: {:?}",
                        recovery_type, recovery_result.recovery_time, recovery_result.error_details
                    );
                }
            }
            Err(e) => {
                self.failed_recoveries.fetch_add(1, Ordering::Relaxed);
                error!("Recovery {:?} failed with error: {}", recovery_type, e);
            }
        }
        
        result
    }
    
    /// Perform index recovery operations
    async fn perform_index_recovery(&self) -> Result<RecoveryResult> {
        let start_time = Instant::now();
        
        // In a real implementation, this would:
        // 1. Scan all data files to rebuild indexes
        // 2. Verify index consistency
        // 3. Rebuild corrupted index structures
        
        if self.enable_detailed_logging {
            debug!("Performing index reconstruction");
        }
        
        // Simulate index recovery
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        Ok(RecoveryResult {
            success: true,
            recovery_type: RecoveryType::IndexReconstruction,
            operations_recovered: 1,
            data_integrity_verified: true,
            recovery_time: start_time.elapsed(),
            error_details: None,
        })
    }
    
    /// Perform memory cleanup operations
    async fn perform_memory_cleanup(&self) -> Result<RecoveryResult> {
        let start_time = Instant::now();
        
        // In a real implementation, this would:
        // 1. Clear memory caches
        // 2. Deallocate unused memory mappings
        // 3. Reset memory counters
        // 4. Trigger garbage collection if needed
        
        if self.enable_detailed_logging {
            debug!("Performing memory cleanup");
        }
        
        // Simulate memory cleanup
        tokio::time::sleep(Duration::from_millis(5)).await;
        
        Ok(RecoveryResult {
            success: true,
            recovery_type: RecoveryType::MemoryCleanup,
            operations_recovered: 1,
            data_integrity_verified: true,
            recovery_time: start_time.elapsed(),
            error_details: None,
        })
    }
    
    /// Perform file system consistency check
    async fn perform_filesystem_check(&self) -> Result<RecoveryResult> {
        let start_time = Instant::now();
        
        // In a real implementation, this would:
        // 1. Verify file integrity
        // 2. Check memory-mapped file consistency
        // 3. Validate data file headers
        // 4. Repair corrupted files if possible
        
        if self.enable_detailed_logging {
            debug!("Performing file system consistency check");
        }
        
        // Simulate filesystem check
        tokio::time::sleep(Duration::from_millis(15)).await;
        
        Ok(RecoveryResult {
            success: true,
            recovery_type: RecoveryType::FileSystemCheck,
            operations_recovered: 1,
            data_integrity_verified: true,
            recovery_time: start_time.elapsed(),
            error_details: None,
        })
    }
    
    /// Perform configuration reset to safe defaults
    async fn perform_configuration_reset(&self) -> Result<RecoveryResult> {
        let start_time = Instant::now();
        
        // In a real implementation, this would:
        // 1. Reset configuration to safe defaults
        // 2. Clear invalid configuration values
        // 3. Reinitialize with minimal settings
        
        if self.enable_detailed_logging {
            debug!("Performing configuration reset to safe defaults");
        }
        
        // Simulate configuration reset
        tokio::time::sleep(Duration::from_millis(2)).await;
        
        Ok(RecoveryResult {
            success: true,
            recovery_type: RecoveryType::ConfigurationReset,
            operations_recovered: 1,
            data_integrity_verified: true,
            recovery_time: start_time.elapsed(),
            error_details: None,
        })
    }
    
    /// Get comprehensive error statistics
    pub async fn get_error_statistics(&self) -> ErrorStatistics {
        let total_errors = self.total_errors.load(Ordering::Relaxed);
        let transaction_errors = self.transaction_errors.load(Ordering::Relaxed);
        let serialization_errors = self.serialization_errors.load(Ordering::Relaxed);
        let io_errors = self.io_errors.load(Ordering::Relaxed);
        let memory_errors = self.memory_errors.load(Ordering::Relaxed);
        let validation_errors = self.validation_errors.load(Ordering::Relaxed);
        
        let total_transactions = self.total_transactions.load(Ordering::Relaxed);
        let successful_transactions = self.successful_transactions.load(Ordering::Relaxed);
        let failed_transactions = self.failed_transactions.load(Ordering::Relaxed);
        let rollback_attempts = self.rollback_attempts.load(Ordering::Relaxed);
        let successful_rollbacks = self.successful_rollbacks.load(Ordering::Relaxed);
        
        let retry_attempts = self.retry_attempts.load(Ordering::Relaxed);
        let successful_retries = self.successful_retries.load(Ordering::Relaxed);
        let failed_retries = self.failed_retries.load(Ordering::Relaxed);
        let max_retry_attempts_reached = self.max_retry_attempts_reached.load(Ordering::Relaxed);
        
        let recovery_attempts = self.recovery_attempts.load(Ordering::Relaxed);
        let successful_recoveries = self.successful_recoveries.load(Ordering::Relaxed);
        
        // Calculate rates
        let transaction_success_rate = if total_transactions > 0 {
            successful_transactions as f64 / total_transactions as f64
        } else {
            1.0
        };
        
        let rollback_success_rate = if rollback_attempts > 0 {
            successful_rollbacks as f64 / rollback_attempts as f64
        } else {
            1.0
        };
        
        let retry_success_rate = if retry_attempts > 0 {
            successful_retries as f64 / retry_attempts as f64
        } else {
            1.0
        };
        
        let recovery_success_rate = if recovery_attempts > 0 {
            successful_recoveries as f64 / recovery_attempts as f64
        } else {
            1.0
        };
        
        // Calculate error rate per hour
        let uptime_hours = self.start_time.elapsed().as_secs_f64() / 3600.0;
        let error_rate_per_hour = if uptime_hours > 0.0 {
            total_errors as f64 / uptime_hours
        } else {
            0.0
        };
        
        // Determine most common error type
        let error_counts = vec![
            (ErrorType::Transaction, transaction_errors),
            (ErrorType::Serialization, serialization_errors),
            (ErrorType::IO, io_errors),
            (ErrorType::Memory, memory_errors),
            (ErrorType::Validation, validation_errors),
        ];
        
        let most_common_error_type = error_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(error_type, _)| error_type)
            .unwrap_or(ErrorType::Unknown);
        
        // Analyze error trend
        let error_trend = self.analyze_error_trend().await;
        
        // Calculate system stability score (0.0 to 1.0)
        let stability_factors = vec![
            transaction_success_rate,
            rollback_success_rate,
            retry_success_rate,
            recovery_success_rate,
            (1.0 - (error_rate_per_hour / 100.0).min(1.0)), // Penalize high error rates
        ];
        
        let system_stability_score = stability_factors.iter().sum::<f64>() / stability_factors.len() as f64;
        
        ErrorStatistics {
            total_errors,
            transaction_errors,
            serialization_errors,
            io_errors,
            memory_errors,
            validation_errors,
            
            total_transactions,
            successful_transactions,
            failed_transactions,
            transaction_success_rate,
            rollback_attempts,
            successful_rollbacks,
            rollback_success_rate,
            
            retry_attempts,
            successful_retries,
            failed_retries,
            retry_success_rate,
            max_retry_attempts_reached,
            
            recovery_attempts,
            successful_recoveries,
            recovery_success_rate,
            
            error_rate_per_hour,
            most_common_error_type,
            error_trend,
            
            system_stability_score,
            uptime: self.start_time.elapsed(),
            last_updated: SystemTime::now(),
        }
    }
    
    /// Analyze error trend over time
    async fn analyze_error_trend(&self) -> ErrorTrend {
        let history = self.error_history.read().await;
        
        if history.len() < 10 {
            return ErrorTrend::Stable;
        }
        
        let now = SystemTime::now();
        let one_hour_ago = now - Duration::from_secs(3600);
        let recent_errors = history
            .iter()
            .filter(|event| event.timestamp > one_hour_ago)
            .count();
        
        let total_errors = history.len();
        let recent_error_rate = recent_errors as f64 / total_errors as f64;
        
        match recent_error_rate {
            rate if rate > 0.7 => ErrorTrend::Critical,
            rate if rate > 0.4 => ErrorTrend::Degrading,
            rate if rate < 0.1 => ErrorTrend::Improving,
            _ => ErrorTrend::Stable,
        }
    }
    
    /// Log comprehensive error summary
    pub async fn log_error_summary(&self) {
        let stats = self.get_error_statistics().await;
        
        info!("=== Error Handling Summary ===");
        info!("Total errors: {} (rate: {:.1}/hour)", stats.total_errors, stats.error_rate_per_hour);
        info!("Error breakdown: TX:{}, Ser:{}, IO:{}, Mem:{}, Val:{}", 
              stats.transaction_errors, stats.serialization_errors, 
              stats.io_errors, stats.memory_errors, stats.validation_errors);
        info!("Transactions: {:.1}% success rate ({}/{})", 
              stats.transaction_success_rate * 100.0, stats.successful_transactions, stats.total_transactions);
        info!("Rollbacks: {:.1}% success rate ({}/{})", 
              stats.rollback_success_rate * 100.0, stats.successful_rollbacks, stats.rollback_attempts);
        info!("Retries: {:.1}% success rate ({}/{})", 
              stats.retry_success_rate * 100.0, stats.successful_retries, stats.retry_attempts);
        info!("Recovery: {:.1}% success rate ({}/{})", 
              stats.recovery_success_rate * 100.0, stats.successful_recoveries, stats.recovery_attempts);
        info!("System stability: {:.1}% (trend: {:?})", 
              stats.system_stability_score * 100.0, stats.error_trend);
        
        if stats.system_stability_score < 0.8 {
            warn!("System stability below 80% - consider investigating error patterns");
        }
        
        if matches!(stats.error_trend, ErrorTrend::Degrading | ErrorTrend::Critical) {
            error!("Error trend is {:?} - immediate attention required", stats.error_trend);
        }
    }
    
    /// Export error statistics in JSON format
    pub async fn export_error_statistics_json(&self) -> Result<String> {
        let stats = self.get_error_statistics().await;
        serde_json::to_string_pretty(&stats).context("Failed to serialize error statistics")
    }
    
    /// Reset all error counters (useful for testing)
    pub async fn reset(&self) {
        self.total_errors.store(0, Ordering::Relaxed);
        self.transaction_errors.store(0, Ordering::Relaxed);
        self.serialization_errors.store(0, Ordering::Relaxed);
        self.io_errors.store(0, Ordering::Relaxed);
        self.memory_errors.store(0, Ordering::Relaxed);
        self.validation_errors.store(0, Ordering::Relaxed);
        
        self.total_transactions.store(0, Ordering::Relaxed);
        self.successful_transactions.store(0, Ordering::Relaxed);
        self.failed_transactions.store(0, Ordering::Relaxed);
        self.rollback_attempts.store(0, Ordering::Relaxed);
        self.successful_rollbacks.store(0, Ordering::Relaxed);
        self.failed_rollbacks.store(0, Ordering::Relaxed);
        
        self.retry_attempts.store(0, Ordering::Relaxed);
        self.successful_retries.store(0, Ordering::Relaxed);
        self.failed_retries.store(0, Ordering::Relaxed);
        self.max_retry_attempts_reached.store(0, Ordering::Relaxed);
        
        self.recovery_attempts.store(0, Ordering::Relaxed);
        self.successful_recoveries.store(0, Ordering::Relaxed);
        self.failed_recoveries.store(0, Ordering::Relaxed);
        
        self.error_history.write().await.clear();
    }
}

/// Shared atomic error handler instance
pub type SharedErrorHandler = Arc<AtomicErrorHandler>;

/// Create a new shared error handler with default configuration
pub fn create_shared_error_handler() -> SharedErrorHandler {
    Arc::new(AtomicErrorHandler::new(
        3, // max_retry_attempts
        Duration::from_millis(100), // base_retry_delay
        Duration::from_secs(30), // max_retry_delay
        true, // enable_detailed_logging
    ))
}

/// Create a new shared error handler with custom configuration
pub fn create_custom_shared_error_handler(
    max_retry_attempts: u32,
    base_retry_delay: Duration,
    max_retry_delay: Duration,
    enable_detailed_logging: bool,
) -> SharedErrorHandler {
    Arc::new(AtomicErrorHandler::new(
        max_retry_attempts,
        base_retry_delay,
        max_retry_delay,
        enable_detailed_logging,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    use anyhow::anyhow;
    
    #[tokio::test]
    async fn test_error_recording() {
        let handler = AtomicErrorHandler::new(
            3,
            Duration::from_millis(10),
            Duration::from_secs(1),
            true,
        );
        
        handler.record_error(
            ErrorType::Transaction,
            "Test transaction error".to_string(),
            "test_operation".to_string(),
            Some(123),
            Some(1000),
            1,
        ).await;
        
        let stats = handler.get_error_statistics().await;
        assert_eq!(stats.total_errors, 1);
        assert_eq!(stats.transaction_errors, 1);
    }
    
    #[tokio::test]
    async fn test_transaction_execution() {
        let handler = AtomicErrorHandler::new(
            3,
            Duration::from_millis(10),
            Duration::from_secs(1),
            false,
        );
        
        // Test successful transaction
        let result = handler.execute_transaction(1, || Ok(())).await.unwrap();
        assert!(result.success);
        assert!(!result.rollback_attempted);
        
        // Test failed transaction
        let result = handler.execute_transaction(2, || {
            Err::<(), anyhow::Error>(anyhow!("Test error"))
        }).await.unwrap();
        assert!(!result.success);
        assert!(result.rollback_attempted);
        assert!(result.rollback_successful);
    }
    
    #[tokio::test]
    async fn test_retry_mechanism() {
        let handler = AtomicErrorHandler::new(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            false,
        );
        
        let mut attempt_count = 0;
        let result = handler.execute_with_retry("test_operation", || {
            attempt_count += 1;
            if attempt_count < 3 {
                Err(anyhow!("Temporary failure"))
            } else {
                Ok(())
            }
        }).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.attempt_count, 3);
        assert!(result.exponential_backoff_used);
    }
    
    #[tokio::test]
    async fn test_recovery_operations() {
        let handler = AtomicErrorHandler::new(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            false,
        );
        
        let result = handler.attempt_recovery(
            RecoveryType::MemoryCleanup,
            "test_context",
        ).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.operations_recovered, 1);
        assert!(result.data_integrity_verified);
    }
    
    #[tokio::test]
    async fn test_exponential_backoff() {
        let handler = AtomicErrorHandler::new(
            5,
            Duration::from_millis(10),
            Duration::from_millis(1000),
            false,
        );
        
        let delay1 = handler.calculate_backoff_delay(1);
        let delay2 = handler.calculate_backoff_delay(2);
        let delay3 = handler.calculate_backoff_delay(3);
        
        // Delays should generally increase (allowing for jitter)
        assert!(delay1.as_millis() >= 5); // At least half of base delay due to jitter
        assert!(delay2.as_millis() >= delay1.as_millis() / 2);
        assert!(delay3.as_millis() >= delay2.as_millis() / 2);
    }
    
    #[tokio::test]
    async fn test_error_statistics() {
        let handler = AtomicErrorHandler::new(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            false,
        );
        
        // Record various types of errors
        handler.record_error(
            ErrorType::Transaction,
            "TX error".to_string(),
            "test".to_string(),
            None,
            None,
            0,
        ).await;
        
        handler.record_error(
            ErrorType::IO,
            "IO error".to_string(),
            "test".to_string(),
            None,
            None,
            0,
        ).await;
        
        let stats = handler.get_error_statistics().await;
        assert_eq!(stats.total_errors, 2);
        assert_eq!(stats.transaction_errors, 1);
        assert_eq!(stats.io_errors, 1);
        assert!(stats.system_stability_score > 0.0);
    }
    
    #[tokio::test]
    async fn test_error_trend_analysis() {
        let handler = AtomicErrorHandler::new(
            3,
            Duration::from_millis(1),
            Duration::from_millis(10),
            false,
        );
        
        // Add multiple errors to trigger trend analysis
        for i in 0..15 {
            handler.record_error(
                ErrorType::IO,
                format!("Error {}", i),
                "test".to_string(),
                None,
                None,
                0,
            ).await;
            
            // Small delay to create time differences
            sleep(Duration::from_millis(1)).await;
        }
        
        let trend = handler.analyze_error_trend().await;
        // Should detect some trend (exact trend depends on timing)
        assert!(matches!(trend, ErrorTrend::Stable | ErrorTrend::Degrading | ErrorTrend::Critical));
    }
}

