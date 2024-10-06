use crate::domain::ports::Chat;
use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessageContentPartImageArgs,
        ChatCompletionRequestMessageContentPartTextArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs, CreateEmbeddingRequest, EmbeddingInput, ImageDetail,
        ImageUrlArgs, Role,
    },
};
use async_trait::async_trait;

const EMBEDDING_MODEL: &str = "mxbai-embed-large";
const BASE_URL: &str = "http://localhost:11434/v1";
const CHAT_MODEL: &str = "llava:13b";

#[derive(Debug, Clone)]
pub struct OpenAI {
    openai_client: async_openai::Client<OpenAIConfig>,
}

impl OpenAI {
    pub fn new() -> Self {
        let openai_config = OpenAIConfig::new().with_api_base(BASE_URL);
        let openai_client = async_openai::Client::with_config(openai_config);

        OpenAI { openai_client }
    }
}

impl Default for OpenAI {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Chat for OpenAI {
    async fn get_chat(
        &self,
        image: String,
        geo_location: Option<String>,
        folder_name: Option<String>,
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
                    .content("Ensure the description is concise, engaging, and not long. Use a maxium of 20 to 30 words.")
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content("Avoid generating a description if the image is unclear. Be confident in the description and do not use words like 'likely' or 'perhaps'.")
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content("Do not refer to the image explicitly. Avoid phrases such as 'This image shows' or 'In this photo'. Focus on describing the essence of the scene directly without any verbs.")
                    .build()?
                    .into(),
            ];

        if geo_location.is_some() {
            let message_content = format!(
                "Use the geolocation {} as a hint where this photo was taken when generating the image summary",
                geo_location.as_ref().unwrap()
            );

            let message = ChatCompletionRequestUserMessageArgs::default()
                .content(message_content)
                .build()?;

            messages.push(message.into());
        }

        if folder_name.is_some() {
            let message_content = format!(
                "Use the folder {} as a hint where this photo was taken when generating the image summary",
                folder_name.as_ref().unwrap()
            );

            let message = ChatCompletionRequestUserMessageArgs::default()
                .content(message_content)
                .build()?;

            messages.push(message.into());
        }

        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model(CHAT_MODEL)
            .messages(messages)
            .build()?;

        tracing::debug!("OpenAI Request: {:?}", request.messages);
        let response = self.openai_client.chat().create(request).await?;
        Ok(process_openai_response(response))
    }

    async fn get_embedding(&self, text: &str) -> Result<std::vec::Vec<f32>> {
        let request = CreateEmbeddingRequest {
            model: EMBEDDING_MODEL.into(),
            input: EmbeddingInput::String(text.into()),
            ..Default::default()
        };

        let response: async_openai::types::CreateEmbeddingResponse =
            match self.openai_client.embeddings().create(request).await {
                Ok(response) => response,
                Err(e) => {
                    panic!("Failed to create embedding: {:?}", e);
                }
            };

        // Extract the first embedding vector from the response
        let embedding = response.data[0].embedding.clone();
        Ok(embedding)
    }
}

fn process_openai_response(response: async_openai::types::CreateChatCompletionResponse) -> String {
    response
        .choices
        .iter()
        .filter_map(|c| {
            if c.message.role == Role::Assistant {
                c.message.content.as_deref()
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
