use simd_json::{prelude::*, Error as JsonError};
use rocksdb::{DB, Options};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageRecord {
    pub id: String,
    pub data: simd_json::OwnedValue,
    pub metadata: simd_json::OwnedValue,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct JsonStorage {
    db: DB,
    json_processor: simd_json::serde::Serializer,
}

impl JsonStorage {
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = DB::open(&opts, path)?;
        
        Ok(Self {
            db,
            json_processor: simd_json::serde::Serializer::new(),
        })
    }

    pub fn store_json(&self, key: &str, data: &simd_json::OwnedValue) -> Result<(), JsonError> {
        let record = StorageRecord {
            id: key.to_string(),
            data: data.clone(),
            metadata: json!({"version": 1, "format": "simd-json"}),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };

        let serialized = simd_json::to_vec(&record)?;
        self.db.put(key.as_bytes(), serialized)?;
        Ok(())
    }

    pub fn get_json(&self, key: &str) -> Result<Option<simd_json::OwnedValue>, JsonError> {
        match self.db.get(key.as_bytes())? {
            Some(bytes) => {
                let mut byte_vec = bytes.to_vec();
                let record: StorageRecord = simd_json::from_slice(&mut byte_vec)?;
                Ok(Some(record.data))
            }
            None => Ok(None),
        }
    }

    pub fn process_stored_json<F>(&self, key: &str, processor: F) -> Result<(), JsonError> 
    where 
        F: FnOnce(&mut simd_json::OwnedValue) -> Result<(), JsonError>
    {
        if let Some(mut data) = self.get_json(key)? {
            processor(&mut data)?;
            self.store_json(key, &data)?;
        }
        Ok(())
    }

    pub async fn load_from_file(&self, file_path: &str) -> Result<simd_json::OwnedValue, JsonError> {
        let contents = fs::read_to_string(file_path).await?;
        let mut contents_mut = contents.clone();
        simd_json::from_str(&mut contents_mut)
    }

    pub async fn save_to_file(&self, file_path: &str, data: &simd_json::OwnedValue) -> Result<(), JsonError> {
        let serialized = simd_json::to_string_pretty(data)?;
        fs::write(file_path, serialized).await?;
        Ok(())
    }

    pub fn scan_all(&self) -> Result<Vec<(String, simd_json::OwnedValue)>, JsonError> {
        let mut results = Vec::new();
        let iter = self.db.iterator(rocksdb::IteratorMode::Start);
        
        for item in iter {
            let (key, value) = item?;
            let key_str = String::from_utf8_lossy(&key).to_string();
            let mut value_vec = value.to_vec();
            let record: StorageRecord = simd_json::from_slice(&mut value_vec)?;
            results.push((key_str, record.data));
        }
        
        Ok(results)
    }
}

// Migration helpers for storage operations
pub fn serialize_for_storage<T: Serialize>(data: &T) -> Result<Vec<u8>, JsonError> {
    simd_json::to_vec(data)
}

pub fn deserialize_from_storage<T: for<'a> Deserialize<'a>>(data: &mut [u8]) -> Result<T, JsonError> {
    simd_json::from_slice(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_operations() {
        let storage = JsonStorage::new("/tmp/test_db").unwrap();
        
        let test_data = json!({
            "name": "Test Item",
            "value": 42,
            "nested": {
                "array": [1, 2, 3],
                "bool": true
            }
        });

        // Store
        storage.store_json("test_key", &test_data).unwrap();
        
        // Retrieve
        let retrieved = storage.get_json("test_key").unwrap().unwrap();
        assert_eq!(retrieved["name"], "Test Item");
        assert_eq!(retrieved["value"], 42);
        assert_eq!(retrieved["nested"]["array"][1], 2);
        
        // Process
        storage.process_stored_json("test_key", |data| {
            data["processed"] = json!(true);
            Ok(())
        }).unwrap();
        
        let processed = storage.get_json("test_key").unwrap().unwrap();
        assert_eq!(processed["processed"], true);
    }
}