use async_trait::async_trait;
use std::path::Path;

use crate::error::{AgixError, Result};
use crate::sources::{FetchOutcome, Source, SourceScheme};

pub struct MarketplaceSource {
    pub marketplace: String,
    pub plugin: String,
}

#[async_trait]
impl Source for MarketplaceSource {
    fn scheme(&self) -> &'static str {
        "marketplace"
    }

    fn canonical(&self) -> String {
        format!("marketplace:{}@{}", self.marketplace, self.plugin)
    }

    fn suggested_name(&self) -> Result<String> {
        Ok(self.plugin.clone())
    }

    async fn fetch(&self, _dest: &Path) -> Result<FetchOutcome> {
        Ok(FetchOutcome::DelegateToDriver {
            marketplace: self.marketplace.clone(),
            plugin: self.plugin.clone(),
        })
    }

    fn as_marketplace(&self) -> Option<(&str, &str)> {
        Some((&self.marketplace, &self.plugin))
    }
}

pub struct MarketplaceScheme;

impl SourceScheme for MarketplaceScheme {
    fn scheme(&self) -> &'static str {
        "marketplace"
    }

    fn parse(&self, value: &str) -> Result<Box<dyn Source>> {
        let (marketplace, plugin) = value.split_once('@').ok_or_else(|| {
            AgixError::InvalidSource(format!(
                "marketplace source must be 'marketplace:<org/repo>@<plugin>', got: marketplace:{value}"
            ))
        })?;
        Ok(Box::new(MarketplaceSource {
            marketplace: marketplace.to_owned(),
            plugin: plugin.to_owned(),
        }))
    }
}
