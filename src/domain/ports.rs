use super::models::{VectorInput, VectorOutput, VectorOutputList};
use anyhow::Result;
use async_trait::async_trait;
use std::{collections::HashMap, path::Path, vec::Vec};

#[async_trait]
pub trait Chat: 'static + Send + Sync {
    /// Asynchronously generates a description for a given base64 encoded image.
    ///
    /// # Arguments
    ///
    /// * `image_base64` - A string slice that contains the base64 encoded image.
    /// * `persons` - A slice of strings that contains the names of people in the image.
    /// * `folder_name` - An optional string slice that represents a folder name for context.
    ///
    /// # Returns
    ///
    /// * `Result<String>` - A Result containing a String that represents the description of the image, or an error.
    async fn get_image_description(
        &self,
        image_base64: &str,
        persons: &[String],
        folder_name: &Option<String>,
    ) -> Result<String>;

    /// Asynchronously generates embeddings for a given list of texts.
    ///
    /// # Arguments
    ///
    /// * `texts` - A vector of strings for which to generate embeddings.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Vec<f32>>>` - A Result containing a vector of float vectors that represent the embeddings, or an error.
    async fn get_embeddings(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>>;

    /// Asynchronously processes the results of a search query and returns a response.
    ///
    /// # Arguments
    ///
    /// * `question` - A string slice that contains the search query.
    /// * `options` - A slice of strings that contain additional context or parameters for the search.
    ///
    /// # Returns
    ///
    /// * `Result<String>` - A Result containing a String that represents the response to the search query, or an error.
    async fn process_search_result(&self, question: &str, options: &[String]) -> Result<String>;
}

/// A trait for encoding images into base64 strings.
pub trait ImageEncoder: 'static + Send + Sync {
    /// Resizes an image and encodes it into a base64 string.
    ///
    /// # Arguments
    ///
    /// * `image_path` - A reference to the path of the image to be resized and encoded.
    ///
    /// # Returns
    ///
    /// * `Result<String>` - A Result containing a String that represents the base64 encoded image, or an error.
    fn resize_and_base64encode_image(&self, image_path: &Path) -> Result<String>;
}

/// A trait for working with XMP metadata in images.
pub trait XMPMetadata: 'static + Send + Sync {
    /// Retrieves the description metadata from an image.
    ///
    /// # Arguments
    ///
    /// * `path` - A reference to the path of the image from which to retrieve the description metadata.
    ///
    /// # Returns
    ///
    /// * `Result<Option<String>>` - A Result containing an Option that represents the description metadata, or an error.
    fn get_description(&self, path: &Path) -> Result<Option<String>>;

    /// Retrieves the geolocation metadata from an image.
    ///
    /// # Arguments
    ///
    /// * `path` - A reference to the path of the image from which to retrieve the geolocation metadata.
    ///
    /// # Returns
    ///
    /// * `Result<Option<String>>` - A Result containing an Option that represents the geolocation metadata, or an error.
    fn get_geolocation(&self, path: &Path) -> Result<Option<String>>;

    /// Sets the description metadata for an image.
    ///
    /// # Arguments
    ///
    /// * `text` - A string slice that contains the description metadata to be set.
    /// * `path` - A reference to the path of the image for which to set the description metadata.
    ///
    /// # Returns
    ///
    /// * `Result<()>` - A Result indicating success or an error.
    fn set_description(&self, text: &str, path: &Path) -> Result<()>;

    /// Retrieves the list of persons mentioned in the image metadata.
    ///
    /// # Arguments
    ///
    /// * `path` - A reference to the path of the image from which to retrieve the persons metadata.
    ///
    /// # Returns
    ///
    /// * `Result<Vec<String>>` - A Result containing a vector of strings that represent the persons mentioned in the image metadata, or an error.
    fn get_persons(&self, path: &Path) -> Result<Vec<String>>;
}

/// A trait for working with vector databases.
#[async_trait]
pub trait VectorDB: 'static + Sync + Send {
    /// Asynchronously creates a collection in the vector database.
    ///
    /// # Arguments
    ///
    /// * `collection` - A string slice that represents the name of the collection to be created.
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - A Result containing a boolean that indicates whether the collection was successfully created, or an error.
    async fn create_collection(&self, collection: &str) -> Result<bool>;

    /// Asynchronously deletes a collection from the vector database.
    ///
    /// # Arguments
    ///
    /// * `text` - A string slice that represents the name of the collection to be deleted.
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - A Result containing a boolean that indicates whether the collection was successfully deleted, or an error.
    async fn delete_collection(&self, text: &str) -> Result<bool>;

    /// Asynchronously upserts points into a collection in the vector database.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - A string slice that represents the name of the collection into which to upsert the points.
    /// * `inputs` - A slice of VectorInput structures that represent the points to be upserted.
    ///
    /// # Returns
    ///
    /// * `Result<bool>` - A Result containing a boolean that indicates whether the points were successfully upserted, or an error.
    async fn upsert_points(&self, collection_name: &str, inputs: &[VectorInput]) -> Result<bool>;

    /// Asynchronously searches for points in a collection in the vector database.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - A string slice that represents the name of the collection to be searched.
    /// * `input_vectors` - A slice of floats that represent the vectors to be searched for.
    /// * `payload_required` - A HashMap that contains the necessary payload for the search.
    ///
    /// # Returns
    ///
    /// * `Result<VectorOutputList>` - A Result containing a VectorOutputList that represents the search results, or an error.
    async fn search_points(
        &self,
        collection_name: &str,
        input_vectors: &[f32],
        payload_required: HashMap<String, String>,
    ) -> Result<VectorOutputList>;

    /// Asynchronously finds a point in a collection in the vector database by its ID.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - A string slice that represents the name of the collection to be searched.
    /// * `id` - A reference to the ID of the point to be found.
    ///
    /// # Returns
    ///
    /// * `Result<Option<VectorOutput>>` - A Result containing an Option that represents the point found, or an error.
    async fn find_by_id(&self, collection_name: &str, id: &u64) -> Result<Option<VectorOutput>>;
}
