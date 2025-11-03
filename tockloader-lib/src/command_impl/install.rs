use async_trait::async_trait;

use crate::connection::TockloaderConnection;
use crate::errors::TockloaderError;
use crate::tabs::tab::Tab;
use crate::CommandInstall;

#[async_trait]
impl CommandInstall for TockloaderConnection {
    async fn install_app(&mut self, _tab: Tab) -> Result<(), TockloaderError> {
        todo!()
    }
}
