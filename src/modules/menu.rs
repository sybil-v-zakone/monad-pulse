use crate::{Result, config::Config};
use database::{db::generate, repositories::create_repositories};
use dialoguer::{Select, theme::ColorfulTheme};
use std::sync::Arc;

use super::warmup::run_warmup;

pub async fn menu() -> Result<()> {
    let repo = Arc::new(create_repositories().await?);
    let config = Arc::new(Config::read_default().await);

    let random_count_range: [u32; 2] = [1, 5];

    loop {
        let options = vec!["DB: Generate", "Warmup", "Exit"];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Choice:")
            .default(0)
            .items(&options)
            .interact()?;

        match selection {
            0 => generate(Arc::clone(&repo), random_count_range).await?,
            1 => run_warmup(Arc::clone(&repo), Arc::clone(&config)).await?,
            2 => return Ok(()),
            _ => tracing::error!("Invalid selection"),
        }
    }
}
