//! Qdrant Edge storage wrapper

use std::collections::HashMap;
use std::path::Path;

use edge::EdgeShard;
use segment::data_types::vectors::{NamedQuery, VectorInternal, VectorStructInternal};
use segment::types::{
    Distance, ExtendedPointId, Payload, PayloadStorageType, SegmentConfig, VectorDataConfig,
    VectorStorageType, WithPayloadInterface, WithVector,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shard::count::CountRequestInternal;
use shard::operations::CollectionUpdateOperations;
use shard::operations::point_ops::{
    PointInsertOperationsInternal, PointOperations, PointStructPersisted,
};
use shard::query::query_enum::QueryEnum;
use shard::query::{ScoringQuery, ShardQueryRequest};

use crate::{Config, QsError, Result, discover};

/// Vector name used in the shard
const VECTOR_NAME: &str = "chunks";

/// Metadata stored with each vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPayload {
    /// Relative file path from repo root
    pub path: String,
    /// Chunk index within the file
    pub chunk_index: usize,
    /// Starting line number
    pub start_line: usize,
    /// Ending line number
    pub end_line: usize,
    /// The actual text content
    pub text: String,
    /// File hash for change detection
    pub file_hash: String,
}

/// A search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Score (similarity)
    pub score: f32,
    /// The payload
    pub payload: ChunkPayload,
}

/// Storage wrapper around Qdrant Edge.
pub struct Storage {
    shard: EdgeShard,
}

impl Storage {
    /// Initialize or load storage for a qs repository.
    pub fn open(root: &Path, config: &Config) -> Result<Self> {
        let shard_path = discover::shard_dir(root);
        std::fs::create_dir_all(&shard_path)?;

        // Create segment config for the shard
        let mut vector_data = HashMap::new();
        vector_data.insert(
            VECTOR_NAME.to_string(),
            VectorDataConfig {
                size: config.dimension,
                distance: Distance::Cosine,
                storage_type: VectorStorageType::ChunkedMmap,
                index: Default::default(),
                quantization_config: None,
                multivector_config: None,
                datatype: None,
            },
        );

        let segment_config = SegmentConfig {
            vector_data,
            sparse_vector_data: HashMap::new(),
            payload_storage_type: PayloadStorageType::Mmap,
        };

        let shard = EdgeShard::load(&shard_path, Some(segment_config))
            .map_err(|e| QsError::Storage(e.to_string()))?;

        Ok(Self { shard })
    }

    /// Insert or update vectors.
    pub fn upsert(&self, points: Vec<(u64, Vec<f32>, ChunkPayload)>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        let point_structs: Vec<PointStructPersisted> = points
            .into_iter()
            .map(|(id, vector, payload)| {
                let payload_json = serde_json::to_value(&payload).unwrap();
                make_point(id, vector, payload_json)
            })
            .collect();

        let operation = CollectionUpdateOperations::PointOperation(PointOperations::UpsertPoints(
            PointInsertOperationsInternal::PointsList(point_structs),
        ));

        self.shard
            .update(operation)
            .map_err(|e| QsError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Search for similar vectors.
    pub fn search(&self, query: Vec<f32>, limit: usize) -> Result<Vec<SearchResult>> {
        let query_vec: VectorInternal = query.into();

        let results = self
            .shard
            .query(ShardQueryRequest {
                prefetches: vec![],
                query: Some(ScoringQuery::Vector(QueryEnum::Nearest(NamedQuery {
                    query: query_vec,
                    using: Some(VECTOR_NAME.to_string()),
                }))),
                filter: None,
                score_threshold: None,
                limit,
                offset: 0,
                params: None,
                with_vector: WithVector::Bool(false),
                with_payload: WithPayloadInterface::Bool(true),
            })
            .map_err(|e| QsError::Storage(e.to_string()))?;

        let search_results = results
            .into_iter()
            .filter_map(|scored| {
                let payload_map = scored.payload?;
                payload_to_chunk(&payload_map)
                    .ok()
                    .map(|payload| SearchResult {
                        score: scored.score,
                        payload,
                    })
            })
            .collect();

        Ok(search_results)
    }

    /// Delete points by IDs.
    pub fn delete(&self, ids: Vec<u64>) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let operation = CollectionUpdateOperations::PointOperation(PointOperations::DeletePoints {
            ids: ids.into_iter().map(ExtendedPointId::NumId).collect(),
        });

        self.shard
            .update(operation)
            .map_err(|e| QsError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Get the number of indexed points.
    pub fn count(&self) -> Result<usize> {
        let count = self
            .shard
            .count(CountRequestInternal {
                filter: None,
                exact: true,
            })
            .map_err(|e| QsError::Storage(e.to_string()))?;

        Ok(count)
    }

    /// Get approximate count from shard info.
    pub fn info_count(&self) -> usize {
        self.shard.info().points_count
    }

    /// Flush all data to disk.
    pub fn flush(&self) {
        self.shard.flush();
    }
}

/// Create a point struct for upserting.
fn make_point(id: u64, vector: Vec<f32>, payload: Value) -> PointStructPersisted {
    let mut vectors = HashMap::new();
    vectors.insert(VECTOR_NAME.to_string(), VectorInternal::from(vector));

    PointStructPersisted {
        id: ExtendedPointId::NumId(id),
        vector: VectorStructInternal::Named(vectors).into(),
        payload: Some(json_to_payload(payload)),
    }
}

/// Convert JSON value to Qdrant Payload.
fn json_to_payload(value: Value) -> Payload {
    if let Value::Object(map) = value {
        let mut payload = Payload::default();
        for (k, v) in map {
            payload.0.insert(k, v);
        }
        payload
    } else {
        Payload::default()
    }
}

/// Convert Qdrant Payload back to ChunkPayload.
fn payload_to_chunk(payload: &Payload) -> Result<ChunkPayload> {
    let json_map: serde_json::Map<String, Value> = payload
        .0
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let json_value = Value::Object(json_map);
    serde_json::from_value(json_value).map_err(|e| QsError::Storage(e.to_string()))
}
