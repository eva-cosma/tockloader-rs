use async_trait::async_trait;
use probe_rs::flashing::DownloadOptions;
use probe_rs::MemoryInterface;
use tbf_parser::parse::parse_tbf_header_lengths;

use crate::board_settings::BoardSettings;
use crate::connection::{Connection, ProbeRSConnection};
use crate::errors::{InternalError, TockloaderError};
use crate::tabs::tab::Tab;
use crate::CommandInstall;

#[async_trait]
impl CommandInstall for ProbeRSConnection {
    async fn install_app(
        &mut self,
        settings: &BoardSettings,
        tab_file: Tab,
    ) -> Result<(), TockloaderError> {
        if !self.is_open() {
            return Err(InternalError::ConnectionNotOpen.into());
        }
        let session = self.session.as_mut().expect("Board must be open");

        let mut core = session.core(self.target_info.core)?;

        // TODO(eva-cosma): extract these informations without bootloader
        // TODO(eva-cosma): extract board name and kernel version to verify app compatability

        let mut address = settings.start_address;

        // TODO(eva-cosma): double-check/rework this

        // Read a block of 200 8-bit words// Loop to check if there are another apps installed
        loop {
            let mut buff = vec![0u8; 200];
            core.read(address, &mut buff)?;

            let (_ver, _header_len, whole_len) = match parse_tbf_header_lengths(
                &buff[0..8]
                    .try_into()
                    .expect("Buffer length must be at least 8 bytes long."),
            ) {
                Ok((ver, header_len, whole_len)) if header_len != 0 => (ver, header_len, whole_len),
                _ => break, // No more apps
            };
            address += whole_len as u64;
        }

        // TODO(eva-cosma): extract arch(?)
        // TODO(eva-cosma): THIS IS NOT A TOCK ERROR, this is an error due to invalid board settings.
        let arch = settings
            .arch
            .as_ref()
            .ok_or(InternalError::MisconfiguredBoardSettings(
                "architechture".to_owned(),
            ))?;

        let mut binary = tab_file.extract_binary(arch)?;
        let size = binary.len() as u64;

        // Make sure the app is aligned to a multiple of its size
        let multiple = address / size;

        let (new_address, _gap_size) = if multiple * size != address {
            let new_address = ((address + size) / size) * size;
            let gap_size = new_address - address;
            (new_address, gap_size)
        } else {
            (address, 0)
        };

        // TODO(eva-cosma): This point MIGHT mark a good point to split
        // this function (for probe-rs).

        // At this point we no longer need to hold the probe-rs connection
        // to the core, as the flashing is done without it.
        drop(core);

        // Make sure the binary is a multiple of the page size by padding 0xFFs

        // TODO(eva-cosma): check if the page-size differs + support
        // multiple types of page sizes. Possibly make page size a board
        // setting.
        let page_size = 512;
        let needs_padding = binary.len() % page_size != 0;

        if needs_padding {
            let remaining = page_size - (binary.len() % page_size);
            dbg!(remaining);
            for _i in 0..remaining {
                binary.push(0xFF);
            }
        }

        // Get indices of pages that have valid data to write
        let mut valid_pages: Vec<u8> = Vec::new();
        for i in 0..(size as usize / page_size) {
            for b in binary[(i * page_size)..((i + 1) * page_size)]
                .iter()
                .copied()
            {
                if b != 0 {
                    valid_pages.push(i.try_into().unwrap());
                    break;
                }
            }
        }

        // If there are no pages valid, all pages would have been removed,
        // so we write them all
        if valid_pages.is_empty() {
            for i in 0..(size as usize / page_size) {
                valid_pages.push(i.try_into().unwrap());
            }
        }

        // Include a blank page (if exists) after the end of a valid page.
        // There might be a usable 0 on the next page
        let mut ending_pages: Vec<u8> = Vec::new();
        for &i in &valid_pages {
            let mut iter = valid_pages.iter();
            if !iter.any(|&x| x == (i + 1)) && (i + 1) < (size as usize / page_size) as u8 {
                ending_pages.push(i + 1);
            }
        }

        for i in ending_pages {
            valid_pages.push(i);
        }

        for i in valid_pages {
            println!("Writing page number {i}");
            // Create the packet that we send to the bootloader. First four
            // bytes are the address of the page
            let mut pkt = Vec::new();

            // Then the bytes that go into the page
            for b in binary[(i as usize * page_size)..((i + 1) as usize * page_size)]
                .iter()
                .copied()
            {
                pkt.push(b);
            }
            let mut loader = session.target().flash_loader();

            loader.add_data(
                (new_address as u32 + (i as usize * page_size) as u32).into(),
                &pkt,
            )?;

            let mut options = DownloadOptions::default();
            options.keep_unwritten_bytes = true;

            // Finally, the data can be programmed
            // TODO(eva-cosma): Can we move this outside the loop? Commit once?
            loader.commit(session, options)?;
        }

        Ok(())
    }
}
