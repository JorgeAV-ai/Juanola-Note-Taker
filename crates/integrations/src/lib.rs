//! Integraciones salientes: Slack, Notion, HubSpot, Attio, Affinity, Zapier
//! (webhooks), Gmail (drafts/send), Google Calendar (events.insert).

#[async_trait::async_trait]
pub trait NoteSink: Send + Sync {
    fn id(&self) -> &'static str;
    async fn send(&self, title: &str, markdown: &str, link: Option<&str>) -> anyhow::Result<()>;
}

pub mod slack {}
pub mod notion {}
pub mod hubspot {}
pub mod attio {}
pub mod affinity {}
pub mod zapier {}
pub mod gmail {}
