pub mod builder;
pub mod types;
pub mod methods;
pub mod handlers;

#[async_trait::async_trait]
pub trait Handler {
    async fn handle_segmented_frame(&mut self, buffer: &[u8]);
}