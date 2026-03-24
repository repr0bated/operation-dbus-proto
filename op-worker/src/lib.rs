use simd_json::{prelude::*, Error as JsonError};
use json_benchmark::JsonProcessor;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub job_type: String,
    pub payload: simd_json::OwnedValue,
    pub priority: u32,
    pub created_at: i64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResult {
    pub item_id: String,
    pub success: bool,
    pub result: Option<simd_json::OwnedValue>,
    pub error: Option<String>,
    pub processing_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStats {
    pub processed_count: u64,
    pub error_count: u64,
    pub avg_processing_time_ms: f64,
    pub last_processed: Option<i64>,
}

pub struct JsonWorker {
    id: String,
    processor: Arc<JsonProcessor>,
    work_queue: Arc<Mutex<mpsc::Receiver<WorkItem>>>,
    result_sender: mpsc::Sender<WorkResult>,
    stats: Arc<Mutex<WorkerStats>>,
}

impl JsonWorker {
    pub fn new(
        id: String,
        work_queue: mpsc::Receiver<WorkItem>,
        result_sender: mpsc::Sender<WorkResult>,
    ) -> Self {
        Self {
            id,
            processor: Arc::new(JsonProcessor::new(16384)),
            work_queue: Arc::new(Mutex::new(work_queue)),
            result_sender,
            stats: Arc::new(Mutex::new(WorkerStats {
                processed_count: 0,
                error_count: 0,
                avg_processing_time_ms: 0.0,
                last_processed: None,
            })),
        }
    }

    pub async fn run(&self) {
        println!("Worker {} starting...", self.id);
        
        loop {
            let mut queue = self.work_queue.lock().await;
            
            match queue.recv().await {
                Some(work_item) => {
                    drop(queue); // Release lock before processing
                    self.process_item(work_item).await;
                }
                None => {
                    println!("Worker {}: No more work, shutting down", self.id);
                    break;
                }
            }
        }
    }

    async fn process_item(&self, item: WorkItem) {
        let start = std::time::Instant::now();
        println!("Worker {} processing item {}", self.id, item.id);

        let result = match self.execute_work(&item).await {
            Ok(result_data) => WorkResult {
                item_id: item.id.clone(),
                success: true,
                result: Some(result_data),
                error: None,
                processing_time_ms: start.elapsed().as_millis() as u64,
            },
            Err(error) => WorkResult {
                item_id: item.id.clone(),
                success: false,
                result: None,
                error: Some(error.to_string()),
                processing_time_ms: start.elapsed().as_millis() as u64,
            }
        };

        // Update stats
        let mut stats = self.stats.lock().await;
        if result.success {
            stats.processed_count += 1;
        } else {
            stats.error_count += 1;
        }
        
        let total_time = stats.avg_processing_time_ms * (stats.processed_count + stats.error_count - 1) as f64;
        stats.avg_processing_time_ms = (total_time + result.processing_time_ms as f64) / (stats.processed_count + stats.error_count) as f64;
        stats.last_processed = Some(chrono::Utc::now().timestamp());
        drop(stats);

        // Send result
        if let Err(e) = self.result_sender.send(result).await {
            eprintln!("Worker {} failed to send result: {}", self.id, e);
        }
    }

    async fn execute_work(&self, item: &WorkItem) -> Result<simd_json::OwnedValue, Box<dyn std::error::Error>> {
        match item.job_type.as_str() {
            "transform" => self.execute_transform(&item.payload).await,
            "validate" => self.execute_validate(&item.payload).await,
            "aggregate" => self.execute_aggregate(&item.payload).await,
            "filter" => self.execute_filter(&item.payload).await,
            _ => Err(format!("Unknown job type: {}", item.job_type).into()),
        }
    }

    async fn execute_transform(&self, payload: &simd_json::OwnedValue) -> Result<simd_json::OwnedValue, Box<dyn std::error::Error>> {
        // Simulate transformation work
        sleep(Duration::from_millis(10)).await;
        
        let mut result = payload.clone();
        
        // Add transformation metadata
        result["transformed"] = json!(true);
        result["worker_id"] = json!(&self.id);
        result["transformed_at"] = json!(chrono::Utc::now().timestamp());
        
        Ok(result)
    }

    async fn execute_validate(&self, payload: &simd_json::OwnedValue) -> Result<simd_json::OwnedValue, Box<dyn std::error::Error>> {
        // Simulate validation work
        sleep(Duration::from_millis(5)).await;
        
        let is_valid = payload.is_object() && !payload.is_empty();
        
        Ok(json!({
            "valid": is_valid,
            "validation_time": chrono::Utc::now().timestamp(),
            "schema_check": true
        }))
    }

    async fn execute_aggregate(&self, payload: &simd_json::OwnedValue) -> Result<simd_json::OwnedValue, Box<dyn std::error::Error>> {
        // Simulate aggregation work
        sleep(Duration::from_millis(15)).await;
        
        let mut sum = 0i64;
        let mut count = 0usize;
        
        if let Some(values) = payload.as_array() {
            for value in values {
                if let Some(num) = value.as_i64() {
                    sum += num;
                    count += 1;
                }
            }
        }
        
        Ok(json!({
            "sum": sum,
            "count": count,
            "average": if count > 0 { sum as f64 / count as f64 } else { 0.0 },
            "aggregated_at": chrono::Utc::now().timestamp()
        }))
    }

    async fn execute_filter(&self, payload: &simd_json::OwnedValue) -> Result<simd_json::OwnedValue, Box<dyn std::error::Error>> {
        // Simulate filtering work
        sleep(Duration::from_millis(8)).await;
        
        let mut filtered = Vec::new();
        
        if let Some(items) = payload.as_array() {
            for item in items {
                // Simple filter: keep objects with "keep" field set to true
                if let Some(keep) = item.get("keep").and_then(|v| v.as_bool()) {
                    if keep {
                        filtered.push(item.clone());
                    }
                }
            }
        }
        
        Ok(json!({
            "filtered_items": filtered,
            "original_count": items.len(),
            "filtered_count": filtered.len(),
            "filtered_at": chrono::Utc::now().timestamp()
        }))
    }

    pub async fn get_stats(&self) -> WorkerStats {
        self.stats.lock().await.clone()
    }
}

pub struct WorkerPool {
    workers: Vec<tokio::task::JoinHandle<()>>,
    work_sender: mpsc::Sender<WorkItem>,
    result_receiver: Arc<Mutex<mpsc::Receiver<WorkResult>>>,
}

impl WorkerPool {
    pub fn new(worker_count: usize, queue_size: usize) -> Self {
        let (work_sender, work_receiver) = mpsc::channel(queue_size);
        let (result_sender, result_receiver) = mpsc::channel(queue_size * 2);
        
        let work_receiver = Arc::new(Mutex::new(work_receiver));
        let result_receiver = Arc::new(Mutex::new(result_receiver));
        
        let mut workers = Vec::new();
        
        for i in 0..worker_count {
            let worker = JsonWorker::new(
                format!("worker-{}", i),
                work_receiver.clone(),
                result_sender.clone(),
            );
            
            let handle = tokio::spawn(async move {
                worker.run().await;
            });
            
            workers.push(handle);
        }
        
        Self {
            workers,
            work_sender,
            result_receiver,
        }
    }

    pub async fn submit_work(&self, item: WorkItem) -> Result<(), tokio::sync::mpsc::error::SendError<WorkItem>> {
        self.work_sender.send(item).await
    }

    pub async fn get_results(&self) -> Option<WorkResult> {
        let mut receiver = self.result_receiver.lock().await;
        receiver.recv().await
    }

    pub async fn shutdown(self) {
        drop(self.work_sender); // Close the channel
        
        for worker in self.workers {
            let _ = worker.await;
        }
    }
}

// Migration helpers for worker operations
pub fn create_work_item(id: String, job_type: String, payload: simd_json::OwnedValue) -> WorkItem {
    WorkItem {
        id,
        job_type,
        payload,
        priority: 0,
        created_at: chrono::Utc::now().timestamp(),
        max_retries: 3,
    }
}

pub fn create_batch_work(items: Vec<(String, String, simd_json::OwnedValue)>) -> Vec<WorkItem> {
    items.into_iter().map(|(id, job_type, payload)| {
        create_work_item(id, job_type, payload)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_migration() {
        let (work_sender, work_receiver) = mpsc::channel(10);
        let (result_sender, result_receiver) = mpsc::channel(10);
        
        let worker = JsonWorker::new(
            "test-worker".to_string(),
            work_receiver,
            result_sender,
        );
        
        // Start worker
        let worker_handle = tokio::spawn(async move {
            worker.run().await;
        });
        
        // Submit work
        let work_item = WorkItem {
            id: "test-1".to_string(),
            job_type: "transform".to_string(),
            payload: json!({"input": "test data"}),
            priority: 1,
            created_at: chrono::Utc::now().timestamp(),
            max_retries: 3,
        };
        
        work_sender.send(work_item).await.unwrap();
        drop(work_sender); // Close channel to stop worker
        
        // Wait for result
        let result = result_receiver.recv().await.unwrap();
        assert_eq!(result.item_id, "test-1");
        assert!(result.success);
        assert!(result.result.is_some());
        
        worker_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_worker_pool() {
        let pool = WorkerPool::new(4, 100);
        
        // Submit multiple work items
        for i in 0..10 {
            let item = WorkItem {
                id: format!("batch-{}", i),
                job_type: "validate".to_string(),
                payload: json!({"item_id": i, "data": format!("test-{}", i)}),
                priority: 0,
                created_at: chrono::Utc::now().timestamp(),
                max_retries: 3,
            };
            pool.submit_work(item).await.unwrap();
        }
        
        // Collect results
        let mut results = Vec::new();
        for _ in 0..10 {
            if let Some(result) = pool.get_results().await {
                results.push(result);
            }
        }
        
        assert_eq!(results.len(), 10);
        for result in &results {
            assert!(result.success);
        }
        
        pool.shutdown().await;
    }
}