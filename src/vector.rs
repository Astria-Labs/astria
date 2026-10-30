use arrow_array::types::Float32Type;
use arrow_array::{Array, FixedSizeListArray, Float32Array, Int32Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use futures::TryStreamExt;
use lancedb::connect;
use lancedb::connection::Connection;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::DistanceType;
use std::sync::Arc;

use crate::parser::CodeChunk;

#[async_trait]
pub trait SemanticIndexer: Send + Sync {
    async fn store_chunks(&self, chunks: &[CodeChunk]) -> Result<(), Box<dyn std::error::Error>>;
    async fn search(&self, query_term: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct VectorStore {
    db: Connection,
    db_path: String,
    model: TextEmbedding,
}

impl VectorStore {
    pub async fn new(db_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let db = lancedb::connect(db_path).execute().await?;

        // Initializes the embeddings local model
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )?;

        Ok(Self {
            db,
            db_path: db_path.to_string(),
            model,
        })
    }
}

#[async_trait]
impl SemanticIndexer for VectorStore {
    async fn store_chunks(&self, chunks: &[CodeChunk]) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::path::Path::new(&self.db_path);
        if path.exists() {
            let _ = std::fs::remove_dir_all(path);
        }

        let db = connect(&self.db_path).execute().await?;

        if chunks.is_empty() {
            println!("No chunks to index.");
            return Ok(());
        }

        let mut texts_to_embed = Vec::new();
        let mut files = Vec::new();
        let mut names = Vec::new();
        let mut kinds = Vec::new();
        let mut start_lines = Vec::new();
        let mut end_lines = Vec::new();

        for chunk in chunks {
            let combined_text = format!(
                "{} {} in {}: {}",
                chunk.kind, chunk.name, chunk.file, chunk.content
            );
            texts_to_embed.push(combined_text);

            files.push(chunk.file.clone());
            names.push(chunk.name.clone());
            kinds.push(chunk.kind.clone());
            start_lines.push(chunk.start_line as i32);
            end_lines.push(chunk.end_line as i32);
        }

        // Chamada direta e síncrona do fastembed (sem block_on)
        let embeddings = self.model.embed(texts_to_embed, None)?;

        if embeddings.is_empty() {
            return Err("Falha ao gerar embeddings para os chunks de código.".into());
        }

        let dim = embeddings[0].len() as i32;

        let schema = Arc::new(Schema::new(vec![
            Field::new("file", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("kind", DataType::Utf8, false),
            Field::new("start_line", DataType::Int32, false),
            Field::new("end_line", DataType::Int32, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                false,
            ),
        ]));

        let file_array = Arc::new(StringArray::from(files)) as Arc<dyn Array>;
        let name_array = Arc::new(StringArray::from(names)) as Arc<dyn Array>;
        let kind_array = Arc::new(StringArray::from(kinds)) as Arc<dyn Array>;
        let start_array = Arc::new(Int32Array::from(start_lines)) as Arc<dyn Array>;
        let end_array = Arc::new(Int32Array::from(end_lines)) as Arc<dyn Array>;

        let vector_data: Vec<Option<Vec<Option<f32>>>> = embeddings
            .into_iter()
            .map(|vec| Some(vec.into_iter().map(Some).collect()))
            .collect();

        let vector_array = Arc::new(
            FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(vector_data, dim),
        ) as Arc<dyn Array>;

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                file_array,
                name_array,
                kind_array,
                start_array,
                end_array,
                vector_array,
            ],
        )?;

        let table_name = "code_index";
        let table_names = db.table_names().execute().await?;
        let batches = arrow_array::RecordBatchIterator::new(vec![Ok(batch)], schema.clone());

        if table_names.contains(&table_name.to_string()) {
            let table = db.open_table(table_name).execute().await?;
            table.add(Box::new(batches)).execute().await?;
        } else {
            db.create_table(table_name, Box::new(batches))
                .execute()
                .await?;
        }

        println!(
            "Successfully indexed {} semantic chunks with local embeddings into LanceDB.",
            chunks.len()
        );
        Ok(())
    }

    async fn search(&self, query_term: &str) -> Result<(), Box<dyn std::error::Error>> {
        let table_names = self.db.table_names().execute().await?;
        if !table_names.contains(&"code_index".to_string()) {
            println!("No semantic index found. Run 'cargo run -- commit' first.");
            return Ok(());
        }

        let table = self.db.open_table("code_index").execute().await?;

        // Chamada direta e síncrona do fastembed para o termo de busca
        let query_embedding = self.model.embed(vec![query_term.to_string()], None)?;

        if query_embedding.is_empty() {
            return Err("Falha ao gerar embedding para o termo de busca.".into());
        }

        let query_vector = query_embedding[0].clone();

        let results = table
            .vector_search(query_vector)?
            .distance_type(DistanceType::Cosine)
            .limit(5)
            .execute()
            .await?;

        let mut found_count = 0;
        let mut stream = results;

        while let Some(batch) = stream.try_next().await? {
            let file_array = batch
                .column_by_name("file")
                .ok_or_else(|| "Missing 'file' column".to_string())?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| "Invalid type for 'file'".to_string())?;

            let name_array = batch
                .column_by_name("name")
                .ok_or_else(|| "Missing 'name' column".to_string())?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| "Invalid type for 'name'".to_string())?;

            let kind_array = batch
                .column_by_name("kind")
                .ok_or_else(|| "Missing 'kind' column".to_string())?
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| "Invalid type for 'kind'".to_string())?;

            let start_array = batch
                .column_by_name("start_line")
                .ok_or_else(|| "Missing 'start_line' column".to_string())?
                .as_any()
                .downcast_ref::<Int32Array>()
                .ok_or_else(|| "Invalid type for 'start_line'".to_string())?;

            let end_array = batch
                .column_by_name("end_line")
                .ok_or_else(|| "Missing 'end_line' column".to_string())?
                .as_any()
                .downcast_ref::<Int32Array>()
                .ok_or_else(|| "Invalid type for 'end_line'".to_string())?;

            let distances = batch
                .column_by_name("_distance")
                .and_then(|col| col.as_any().downcast_ref::<Float32Array>());

            for i in 0..batch.num_rows() {
                found_count += 1;
                let distance = distances.map(|d| d.value(i)).unwrap_or(0.0);

                println!(
                    "  🎯 [Score/Dist: {:.4}] File: {} | [{}] '{}' (Lines {}-{})",
                    distance,
                    file_array.value(i),
                    kind_array.value(i),
                    name_array.value(i),
                    start_array.value(i),
                    end_array.value(i)
                );
            }
        }

        if found_count == 0 {
            println!(
                "No semantically matching code blocks found for '{}'.",
                query_term
            );
        } else {
            println!("\nFound {} semantic match(es).", found_count);
        }

        Ok(())
    }
}
