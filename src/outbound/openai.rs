use crate::domain::ports::Chat;
use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessageContentPartImageArgs,
        ChatCompletionRequestMessageContentPartText,
        ChatCompletionRequestMessageContentPartTextArgs, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionRequestUserMessageContentPart,
        CreateChatCompletionRequestArgs, CreateEmbeddingRequest, CreateEmbeddingResponse,
        EmbeddingInput, ImageDetail, ImageUrlArgs, Role,
    },
};
use async_trait::async_trait;
use std::{env::var, vec::Vec};
use tracing::error;

const EMBEDDING_MODEL: &str = "mxbai-embed-large";
const BASE_URL: &str = "http://localhost:11434/v1";
const CHAT_MODEL_MULTIMODAL: &str = "llava:13b";
const CHAT_MODEL_TEXT: &str = "llama3.1:8b";

#[derive(Debug, Clone, Default)]
pub struct OpenAI {
    openai_client: async_openai::Client<OpenAIConfig>,
    chat_model: String,
    multimodal_model: String,
    embedding_model: String,
}

impl OpenAI {
    pub fn new() -> Self {
        // load env from .env file
        dotenv::dotenv().ok();
        let api_key = var("CHAT_API_KEY").ok();
        let api_base = var("CHAT_API_BASE").unwrap_or(BASE_URL.into());

        let openai_config = OpenAIConfig::new()
            .with_api_base(api_base)
            .with_api_key(api_key.unwrap_or_default());
        let openai_client = async_openai::Client::with_config(openai_config);

        let chat_model = var("CHAT_MODEL").unwrap_or(CHAT_MODEL_TEXT.into());
        let multimodal_model = var("CHAT_MODEL_IMAGE").unwrap_or(CHAT_MODEL_MULTIMODAL.into());
        let embedding_model = var("CHAT_MODEL_EMBEDDINGS").unwrap_or(EMBEDDING_MODEL.into());

        OpenAI {
            openai_client,
            chat_model,
            multimodal_model,
            embedding_model,
        }
    }
}

#[async_trait]
impl Chat for OpenAI {
    async fn get_image_description(
        &self,
        image: &str,
        persons: &[String],
        folder_name: &Option<String>,
    ) -> Result<String> {
        let mut messages = vec![
                ChatCompletionRequestUserMessageArgs::default()
                    .content("You are a traveler immersed in the world around you. Describe the scene with attention to cultural, geographical, and sensory details. Offer personal insights and reflections that reveal the atmosphere, local traditions, and unique experiences of the place. Bring the reader into the moment with vivid descriptions.")
                    .build()?
                    .into(),
                 ChatCompletionRequestUserMessageArgs::default()
                    .content(vec![
                        ChatCompletionRequestMessageContentPartTextArgs::default()
                            .text("The photo: ")
                            .build()?
                            .into(),
                        ChatCompletionRequestMessageContentPartImageArgs::default()
                            .image_url(
                                ImageUrlArgs::default()
                                    .url(format!("data:image/jpeg;base64,{}", image))
                                    .detail(ImageDetail::High)
                                    .build()?,
                            )
                            .build()?
                            .into(),
                        ])
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content("Ensure the description is concise and engaging. Limit the description to 2-3 sentences.")
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content("Avoid generating a description if the image is unclear. Be confident in the description and do not use words like 'likely' or 'perhaps'.")
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content("Do not refer to the image explicitly. Avoid phrases such as 'This image shows' or 'In this photo' or 'This scene'. Focus on describing the essence of the scene directly without any verbs.")
                    .build()?
                    .into(),
            ];

        if !persons.is_empty() {
            let message_content = format!(
                "Use the person(s) {} as a hint who is in the photo when generating the image summary",
                persons.join(", ")
            );

            let message = ChatCompletionRequestUserMessageArgs::default()
                .content(message_content)
                .build()?;

            messages.push(message.into());
        }

        if let Some(folder) = folder_name {
            let message_content = format!(
                    "Use the folder {} as a hint where this photo was taken when generating the image summary",
                    folder
                );

            let message = ChatCompletionRequestUserMessageArgs::default()
                .content(message_content)
                .build()?;

            messages.push(message.into());
        }

        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model(self.multimodal_model.clone())
            .messages(messages)
            .build()?;

        tracing::debug!("OpenAI Request: {:?}", request.messages);
        let response = self.openai_client.chat().create(request).await?;
        Ok(process_openai_response(response))
    }

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = CreateEmbeddingRequest {
            model: self.embedding_model.clone(),
            input: EmbeddingInput::String(text.into()),
            ..Default::default()
        };

        let response: CreateEmbeddingResponse =
            match self.openai_client.embeddings().create(request).await {
                Ok(response) => response,
                Err(e) => {
                    error!("Failed to create embedding: {:?}", e);
                    Err(anyhow::Error::from(e))?
                }
            };

        // Extract the first embedding vector from the response
        let embedding = &response.data[0].embedding;
        Ok(embedding.clone())
    }

    async fn process_search_result(&self, question: &str, options: Vec<String>) -> Result<String> {
        let options: Vec<ChatCompletionRequestUserMessageContentPart> = options
            .iter()
            .map(|o| ChatCompletionRequestMessageContentPartText { text: o.clone() }.into())
            .collect();

        let messages = vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content(
                    "You are a helpful assistant answering the given question using given options.",
                )
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(question)
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(options)
                .build()?
                .into(),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model(self.chat_model.clone())
            .messages(messages)
            .build()?;

        tracing::debug!("OpenAI Request: {:?}", request.messages);
        let response = self.openai_client.chat().create(request).await?;
        Ok(process_openai_response(response))
    }
}

fn process_openai_response(response: async_openai::types::CreateChatCompletionResponse) -> String {
    response
        .choices
        .iter()
        .filter_map(|c| {
            if c.message.role == Role::Assistant {
                c.message.content.as_deref().map(|s| s.trim())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
