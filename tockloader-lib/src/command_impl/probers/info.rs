use async_trait::async_trait;

use crate::attributes::app_attributes::AppAttributes;
use crate::attributes::general_attributes::GeneralAttributes;
use crate::attributes::system_attributes::SystemAttributes;
use crate::board_settings::BoardSettings;
use crate::connection::{Connection, ProbeRSConnection};
use crate::errors::{InternalError, TockloaderError};
use crate::CommandInfo;

#[async_trait]
impl CommandInfo for ProbeRSConnection {
    async fn info(
        &mut self,
        settings: &BoardSettings,
    ) -> Result<GeneralAttributes, TockloaderError> {
        if !self.is_open() {
            return Err(InternalError::ConnectionNotOpen.into());
        }
        let session = self.session.as_mut().expect("Board must be open");

        let mut core = session.core(self.target_info.core)?;

        // TODO(eva-cosma): extract these informations without bootloader
        let system_attributes = SystemAttributes::read_system_attributes_probe(&mut core)?;
        let app_attributes =
            AppAttributes::read_apps_data_probe(&mut core, settings.start_address)?;

        Ok(GeneralAttributes::new(system_attributes, app_attributes))
    }
}
