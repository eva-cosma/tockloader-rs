// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright OXIDOS AUTOMOTIVE 2024.

use probe_rs::{Core, MemoryInterface};

use tbf_parser::parse::{parse_tbf_footer, parse_tbf_header, parse_tbf_header_lengths};
use tbf_parser::types::{TbfFooterV2Credentials, TbfHeader};
use tbf_parser::{self};
use tokio_serial::SerialStream;

use crate::bootloader_serial::{issue_command, Command, Response};
use crate::errors::{TockError, TockloaderError};

/// This structure contains all relevant information about a tock application.
///
/// All data is stored either within [TbfHeader]s, or [TbfFooter]s.
///
/// See also <https://book.tockos.org/doc/tock_binary_format>
#[derive(Debug)]
pub struct AppAttributes {
    pub address: u64,
    pub tbf_header: TbfHeader,
    pub tbf_footers: Vec<TbfFooter>,
}

/// This structure represents a footer of a Tock application. Currently, footers
/// only contain credentials, which are used to verify the integrity of the
/// application.
#[derive(Debug)]
pub struct TbfFooter {
    pub credentials: TbfFooterV2Credentials,
    pub size: u32,
}

impl TbfFooter {
    pub fn new(credentials: TbfFooterV2Credentials, size: u32) -> TbfFooter {
        TbfFooter { credentials, size }
    }
}

// TODO(eva-cosma): Could take advantages of the trait rework

impl AppAttributes {
    pub(crate) fn new(
        address: u64,
        header_data: TbfHeader,
        footers_data: Vec<TbfFooter>,
    ) -> AppAttributes {
        AppAttributes {
            address,
            tbf_header: header_data,
            tbf_footers: footers_data,
        }
    }

    /// Retrieve all application attributes from the device's memory using a
    /// probe-rs connection.
    ///
    /// Applications are layed out in memory sequentially, starting from the
    /// `appaddr` address. This function will attempt to read all applications
    /// until it fails to parse.
    ///
    /// # Parameters
    /// - `board_core`: Core access, obtained from a
    ///   [ProbeRSConnection](crate::connection::ProbeRSConnection)
    /// - `addr`: The starting address of the first application in memory.
    ///   Board-specific. See also
    ///   [BoardSettings](crate::board_settings::BoardSettings).
    pub(crate) fn read_apps_data_probe(
        board_core: &mut Core,
        addr: u64,
    ) -> Result<Vec<AppAttributes>, TockloaderError> {
        let mut appaddr: u64 = addr;
        let mut apps_counter = 0;
        let mut apps_details: Vec<AppAttributes> = vec![];

        // All applications are stored sequentially in memory, so we read until
        // we fail to parse.
        loop {
            let mut appdata = vec![0u8; 8];

            board_core.read(appaddr, &mut appdata)?;

            let tbf_version: u16;
            let header_size: u16;
            let total_size: u32;

            // The first 8 bytes of the application data contain the TBF header
            // lengths and version.
            //
            // Note on expect: `read` always fills up the entire buffer, which
            // was previously declared as 8 bytes.
            match parse_tbf_header_lengths(
                &appdata
                    .try_into()
                    .expect("Buffer length must be at least 8 bytes long."),
            ) {
                Ok(data) => {
                    tbf_version = data.0;
                    header_size = data.1;
                    total_size = data.2;
                }
                _ => return Ok(apps_details),
            };

            log::debug!(
                "App #{apps_counter}: TBF version {tbf_version}, header size {header_size}, total size {total_size}",
            );

            let mut header_data = vec![0u8; header_size as usize];

            board_core.read(appaddr, &mut header_data)?;
            log::debug!("App #{apps_counter}: Header data: {header_data:?}");
            let header = parse_tbf_header(&header_data, tbf_version)
                .map_err(TockError::InvalidAppTbfHeader)?;

            // The end of the application binary marks the beginning of the
            // footer.
            //
            // Note: This is not always true, `get_binary_end`
            // does not make sense if the application is just padding. This can
            // crash the process.
            let binary_end_offset = header.get_binary_end();

            match &header {
                TbfHeader::TbfHeaderV2(_hd) => {}
                _ => {
                    appaddr += total_size as u64;
                    continue;
                }
            };

            let mut footers: Vec<TbfFooter> = vec![];
            let total_footers_size = total_size - binary_end_offset;
            let mut footer_offset = binary_end_offset;
            let mut footer_number = 0;

            // Try to parse footers until we reach the end of the application.
            while footer_offset < total_size {
                // We don't know the size of the current footer, so we read the
                // remaining bytes in the application (`footer_offset -
                // binary_end_offset`) , even if we overread.
                let mut appfooter =
                    vec![0u8; (total_footers_size - (footer_offset - binary_end_offset)) as usize];

                board_core.read(appaddr + footer_offset as u64, &mut appfooter)?;

                let footer_info =
                    parse_tbf_footer(&appfooter).map_err(TockError::InvalidAppTbfHeader)?;

                footers.insert(footer_number, TbfFooter::new(footer_info.0, footer_info.1));

                footer_number += 1;
                // we add 4 because that is the size of TL part of the TLV header (2 bytes type + 2 bytes length)
                footer_offset += footer_info.1 + 4;
            }

            let details: AppAttributes = AppAttributes::new(appaddr, header, footers);

            apps_details.insert(apps_counter, details);
            apps_counter += 1;
            appaddr += total_size as u64;
        }
    }

    /// Retrieve all application attributes from the device's memory using a
    /// serial connection.
    ///
    /// Applications are layed out in memory sequentially, starting from the
    /// `appaddr` address. This function will attempt to read all applications
    /// until it fails to parse.
    ///
    /// # Parameters
    /// - `port`: Serial access, obtained from a
    ///   [SerialConnection](crate::connection::SerialConnection)
    /// - `addr`: The starting address of the first application in memory.
    ///   Board-specific. See also
    ///   [BoardSettings](crate::board_settings::BoardSettings).
    pub(crate) async fn read_apps_data_serial(
        port: &mut SerialStream,
        addr: u64,
    ) -> Result<Vec<AppAttributes>, TockloaderError> {
        let mut appaddr: u64 = addr;
        let mut apps_counter = 0;
        let mut apps_details: Vec<AppAttributes> = vec![];

        // All applications are stored sequentially in memory, so we read until
        // we fail to parse.
        loop {
            // The tockloader protocol only supports 32-bit architectures,
            // though in the future support will be extended to 64-bit.
            let mut pkt = (appaddr as u32).to_le_bytes().to_vec();
            let length = (8_u16).to_le_bytes().to_vec();
            for i in length {
                pkt.push(i);
            }

            // Read the first 8 bytes, which is the length of a TLV header.
            let (_, appdata) =
                issue_command(port, Command::ReadRange, pkt, true, 8, Response::ReadRange).await?;

            let tbf_version: u16;
            let header_size: u16;
            let total_size: u32;

            // The first 8 bytes of the application data contain the TBF header
            // lengths and version.
            //
            // Note on expect: `read` always fills up the entire buffer, which
            // was previously declared as 8 bytes.
            match parse_tbf_header_lengths(
                &appdata[0..8]
                    .try_into()
                    .expect("Buffer length must be at least 8 bytes long."),
            ) {
                Ok(data) => {
                    tbf_version = data.0;
                    header_size = data.1;
                    total_size = data.2;
                }
                _ => break,
            };

            log::debug!(
                "App #{apps_counter}: TBF version {tbf_version}, header size {header_size}, total size {total_size}",
            );

            let mut pkt = (appaddr as u32).to_le_bytes().to_vec();
            let length = (header_size).to_le_bytes().to_vec();
            for i in length {
                pkt.push(i);
            }

            // Read the rest of the header
            let (_, header_data) = issue_command(
                port,
                Command::ReadRange,
                pkt,
                true,
                header_size.into(),
                Response::ReadRange,
            )
            .await?;

            log::debug!("App #{apps_counter}: Header data: {header_data:?}");
            let header = parse_tbf_header(&header_data, tbf_version)
                .map_err(TockError::InvalidAppTbfHeader)?;
            let binary_end_offset = header.get_binary_end();

            match &header {
                TbfHeader::TbfHeaderV2(_hd) => {}
                _ => {
                    appaddr += total_size as u64;
                    continue;
                }
            };

            let mut footers: Vec<TbfFooter> = vec![];
            let total_footers_size = total_size - binary_end_offset;
            let mut footer_offset = binary_end_offset;
            let mut footer_number = 0;

            // Try to parse footers until we reach the end of the application.
            while footer_offset < total_size {
                // We don't know the size of the current footer, so we read the
                // remaining bytes in the application (`footer_offset -
                // binary_end_offset`) , even if we overread.
                let mut pkt = (appaddr as u32 + footer_offset).to_le_bytes().to_vec();
                let length = ((total_footers_size - (footer_offset - binary_end_offset)) as u16)
                    .to_le_bytes()
                    .to_vec();
                for i in length {
                    pkt.push(i);
                }

                // Read the next header (and perhaps data beyond it)
                let (_, appfooter) = issue_command(
                    port,
                    Command::ReadRange,
                    pkt,
                    true,
                    (total_footers_size - (footer_offset - binary_end_offset)) as usize,
                    Response::ReadRange,
                )
                .await?;

                let footer_info =
                    parse_tbf_footer(&appfooter).map_err(TockError::InvalidAppTbfHeader)?;

                footers.insert(footer_number, TbfFooter::new(footer_info.0, footer_info.1));

                footer_number += 1;
                // we add 4 because that is the size of TL part of the TLV header (2 bytes type + 2 bytes length)
                footer_offset += footer_info.1 + 4;
            }

            let details: AppAttributes = AppAttributes::new(appaddr, header, footers);

            apps_details.insert(apps_counter, details);
            apps_counter += 1;
            appaddr += total_size as u64;
        }
        Ok(apps_details)
    }
}
