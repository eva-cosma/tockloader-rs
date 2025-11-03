use async_trait::async_trait;

use crate::attributes::app_attributes::AppAttributes;
use crate::connection::TockloaderConnection;
use crate::errors::TockloaderError;
use crate::{CommandList, IOCommands};

#[async_trait]
impl CommandList for TockloaderConnection {
    async fn list(&mut self) -> Result<Vec<AppAttributes>, TockloaderError> {
        self.read_installed_apps().await
    }
}
