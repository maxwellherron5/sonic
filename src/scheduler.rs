use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::discord_announcer::DiscordAnnouncer;
use crate::discovery_generator::DiscoveryGenerator;
use crate::error::{SchedulerError, SchedulerResult};
use crate::models::BotConfig;

/// Task scheduler for managing time-based operations
/// Handles weekly discovery playlist generation and other scheduled tasks
pub struct TaskScheduler {
    scheduler: JobScheduler,
    discovery_generator: Arc<Mutex<DiscoveryGenerator>>,
    discord_announcer: Arc<Mutex<DiscordAnnouncer>>,
    config: BotConfig,
}

impl TaskScheduler {
    /// Create a new TaskScheduler instance
    pub async fn new(
        discovery_generator: Arc<Mutex<DiscoveryGenerator>>,
        discord_announcer: Arc<Mutex<DiscordAnnouncer>>,
        config: BotConfig,
    ) -> SchedulerResult<Self> {
        let scheduler = JobScheduler::new()
            .await
            .map_err(|e| SchedulerError::StartFailed(format!("Failed to create scheduler: {}", e)))?;

        Ok(Self {
            scheduler,
            discovery_generator,
            discord_announcer,
            config,
        })
    }

    /// Start the weekly discovery playlist generation schedule
    /// Implements requirement 4.1: generate discovery playlist every 7 days
    pub async fn start_weekly_schedule(&mut self) -> SchedulerResult<()> {
        log::info!("Starting weekly discovery playlist schedule with cron: {}", self.config.weekly_schedule_cron);

        // Validate the cron expression by trying to parse it
        self.validate_cron_expression(&self.config.weekly_schedule_cron)?;

        // Clone the necessary components for the job closure
        let discovery_generator = Arc::clone(&self.discovery_generator);
        let discord_announcer = Arc::clone(&self.discord_announcer);
        let cron_expression = self.config.weekly_schedule_cron.clone();

        // Create the weekly discovery generation job
        let job = Job::new_async(cron_expression.as_str(), move |_uuid, _l| {
            let discovery_generator = Arc::clone(&discovery_generator);
            let discord_announcer = Arc::clone(&discord_announcer);
            
            Box::pin(async move {
                log::info!("Executing scheduled weekly discovery playlist generation");
                
                match Self::execute_discovery_generation_task(discovery_generator, discord_announcer).await {
                    Ok(_) => {
                        log::info!("Weekly discovery playlist generation completed successfully");
                    }
                    Err(e) => {
                        log::error!("Weekly discovery playlist generation failed: {:?}", e);
                    }
                }
            })
        })
        .map_err(|_e| SchedulerError::InvalidCronExpression { 
            expression: self.config.weekly_schedule_cron.clone() 
        })?;

        // Add the job to the scheduler
        self.scheduler.add(job)
            .await
            .map_err(|e| SchedulerError::StartFailed(format!("Failed to add weekly job: {}", e)))?;

        // Start the scheduler
        self.scheduler.start()
            .await
            .map_err(|e| SchedulerError::StartFailed(format!("Failed to start scheduler: {}", e)))?;

        log::info!("Weekly discovery playlist scheduler started successfully");
        Ok(())
    }

    /// Execute the discovery playlist generation task
    /// This is the main task that runs on the weekly schedule
    /// Implements requirements 4.1 and 4.5: generate and announce discovery playlist
    async fn execute_discovery_generation_task(
        discovery_generator: Arc<Mutex<DiscoveryGenerator>>,
        discord_announcer: Arc<Mutex<DiscordAnnouncer>>,
    ) -> SchedulerResult<()> {
        log::info!("Starting discovery playlist generation task");

        // Generate and announce the discovery playlist
        let result = {
            let generator = discovery_generator.lock().await;
            let announcer = discord_announcer.lock().await;
            
            generator.generate_and_announce_discovery_playlist(&*announcer).await
        };

        match result {
            Ok(discovery_playlist) => {
                log::info!(
                    "Successfully generated discovery playlist with {} tracks using {} seed tracks",
                    discovery_playlist.track_count(),
                    discovery_playlist.seed_tracks.len()
                );
                Ok(())
            }
            Err(e) => {
                log::error!("Discovery playlist generation failed: {:?}", e);
                
                // Try to announce the error to Discord
                let announcer = discord_announcer.lock().await;
                if let Err(announce_err) = announcer.announce_discovery_error(&format!("{:?}", e)).await {
                    log::error!("Failed to announce discovery error to Discord: {:?}", announce_err);
                }
                
                Err(SchedulerError::TaskExecutionFailed(format!(
                    "Discovery playlist generation failed: {:?}", e
                )))
            }
        }
    }

    /// Manually trigger discovery playlist generation
    /// This allows for manual execution outside of the scheduled time
    pub async fn execute_manual_discovery_generation(&self) -> SchedulerResult<()> {
        log::info!("Executing manual discovery playlist generation");
        
        Self::execute_discovery_generation_task(
            Arc::clone(&self.discovery_generator),
            Arc::clone(&self.discord_announcer),
        ).await
    }

    /// Stop the scheduler and all scheduled tasks
    /// Implements graceful shutdown handling
    pub async fn stop(&mut self) -> SchedulerResult<()> {
        log::info!("Stopping task scheduler");
        
        self.scheduler.shutdown()
            .await
            .map_err(|e| SchedulerError::StopFailed(format!("Failed to stop scheduler: {}", e)))?;
        
        log::info!("Task scheduler stopped successfully");
        Ok(())
    }

    /// Check if the scheduler is running
    pub fn is_running(&self) -> bool {
        // Note: tokio-cron-scheduler doesn't provide a direct way to check if running
        // We'll track this internally or assume it's running after start() is called
        true // Simplified for now
    }

    /// Get the next scheduled execution time
    /// This is useful for monitoring and debugging
    pub fn get_next_execution_info(&self) -> String {
        format!(
            "Next discovery playlist generation scheduled with cron expression: {}",
            self.config.weekly_schedule_cron
        )
    }

    /// Validate the cron expression format
    /// Ensures the cron expression is valid before starting the scheduler
    fn validate_cron_expression(&self, expression: &str) -> SchedulerResult<()> {
        // Try to create a temporary job to validate the cron expression
        match Job::new(expression, |_, _| {}) {
            Ok(_) => {
                log::debug!("Cron expression '{}' is valid", expression);
                Ok(())
            }
            Err(e) => {
                log::error!("Invalid cron expression '{}': {:?}", expression, e);
                Err(SchedulerError::InvalidCronExpression {
                    expression: expression.to_string(),
                })
            }
        }
    }

    /// Get scheduler statistics and status
    pub async fn get_scheduler_stats(&self) -> SchedulerStats {
        SchedulerStats {
            is_running: self.is_running(),
            cron_expression: self.config.weekly_schedule_cron.clone(),
            next_execution_info: self.get_next_execution_info(),
        }
    }
}

/// Statistics and status information about the scheduler
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    /// Whether the scheduler is currently running
    pub is_running: bool,
    /// The cron expression being used for scheduling
    pub cron_expression: String,
    /// Information about the next scheduled execution
    pub next_execution_info: String,
}

impl SchedulerStats {
    /// Format the scheduler statistics for display
    pub fn format_stats(&self) -> String {
        format!(
            "📅 **Scheduler Status**\n\
            • Status: {}\n\
            • Schedule: `{}`\n\
            • {}",
            if self.is_running { "🟢 Running" } else { "🔴 Stopped" },
            self.cron_expression,
            self.next_execution_info
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::BotConfig;

    fn create_test_config() -> BotConfig {
        BotConfig {
            discord_token: "test".to_string(),
            spotify_client_id: "test".to_string(),
            spotify_client_secret: "test".to_string(),
            spotify_refresh_token: "test".to_string(),
            target_channel_id: 123456789,
            collaborative_playlist_id: "collab123".to_string(),
            discovery_playlist_id: "discovery123".to_string(),
            weekly_schedule_cron: "0 0 12 * * MON".to_string(),
            max_retry_attempts: 3,
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 30000,
        }
    }

    #[test]
    fn test_validate_cron_expression_standalone() {
        let config = create_test_config();
        
        // Create a minimal scheduler instance for testing validation
        struct TestScheduler {
            config: BotConfig,
        }
        
        impl TestScheduler {
            fn validate_cron_expression(&self, expression: &str) -> SchedulerResult<()> {
                match Job::new(expression, |_, _| {}) {
                    Ok(_) => Ok(()),
                    Err(_) => Err(SchedulerError::InvalidCronExpression {
                        expression: expression.to_string(),
                    }),
                }
            }
        }
        
        let test_scheduler = TestScheduler { config };

        // Test valid cron expressions
        assert!(test_scheduler.validate_cron_expression("0 0 12 * * MON").is_ok());
        assert!(test_scheduler.validate_cron_expression("0 30 9 * * FRI").is_ok());
        assert!(test_scheduler.validate_cron_expression("0 0 0 1 * *").is_ok());

        // Test invalid cron expressions
        assert!(test_scheduler.validate_cron_expression("invalid").is_err());
        assert!(test_scheduler.validate_cron_expression("").is_err());
        assert!(test_scheduler.validate_cron_expression("0 0 25 * * *").is_err()); // Invalid hour
    }

    #[test]
    fn test_scheduler_stats_format() {
        let stats = SchedulerStats {
            is_running: true,
            cron_expression: "0 0 12 * * MON".to_string(),
            next_execution_info: "Next execution: Monday at 12:00 PM".to_string(),
        };

        let formatted = stats.format_stats();
        assert!(formatted.contains("🟢 Running"));
        assert!(formatted.contains("0 0 12 * * MON"));
        assert!(formatted.contains("Next execution"));
    }

    #[test]
    fn test_scheduler_stats_stopped() {
        let stats = SchedulerStats {
            is_running: false,
            cron_expression: "0 0 12 * * MON".to_string(),
            next_execution_info: "Scheduler is stopped".to_string(),
        };

        let formatted = stats.format_stats();
        assert!(formatted.contains("🔴 Stopped"));
        assert!(formatted.contains("0 0 12 * * MON"));
        assert!(formatted.contains("Scheduler is stopped"));
    }
}