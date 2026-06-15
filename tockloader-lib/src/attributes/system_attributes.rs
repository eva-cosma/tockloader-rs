// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright OXIDOS AUTOMOTIVE 2024.

use byteorder::{ByteOrder, LittleEndian};
use probe_rs::{Core, MemoryInterface};
use tokio_serial::SerialStream;

use crate::bootloader_serial::{issue_command, Command, Response};
use crate::errors::{AttributeParseError, TockError, TockloaderError};

use super::decode::{bytes_to_string, decode_attribute};

/// This structure contains all relevant information about board that is stored
/// in the bootloader ROM.
///
/// Note: not all system attributes are present on all boards. You cannot assume
/// any of these structure members are `Some(_)`.
///
/// See also <https://book.tockos.org/doc/kernel_attributes.html?highlight=attributes#header-format>
#[derive(Debug)]
pub struct SystemAttributes {
    pub board: Option<String>,
    pub arch: Option<String>,
    pub appaddr: Option<u64>,
    pub boothash: Option<String>,
    pub bootloader_version: Option<String>,
    pub sentinel: Option<String>,
    pub kernel_version: Option<u64>,
    pub app_mem_start: Option<u32>,
    pub app_mem_len: Option<u32>,
    pub kernel_bin_start: Option<u32>,
    pub kernel_bin_len: Option<u32>,
}

impl SystemAttributes {
    pub(crate) fn new() -> SystemAttributes {
        SystemAttributes {
            board: None,
            arch: None,
            appaddr: None,
            boothash: None,
            bootloader_version: None,
            sentinel: None,
            kernel_version: None,
            app_mem_start: None,
            app_mem_len: None,
            kernel_bin_start: None,
            kernel_bin_len: None,
        }
    }

    /// Read system attributes using a probe-rs connection. A bootloader must be
    /// present on this board for this function to work properly.
    ///
    /// # Parameters
    /// - `board_core` : Core access, obtained from a
    ///   [ProbeRSConnection](crate::connection::ProbeRSConnection)
    ///
    /// # Returns
    /// - Ok(result): if attributes were read successfully
    /// - Err(TockloaderError::MisconfiguredBoard): if no start address is found or valid
    /// - Err(TockloaderError::MisconfiguredBoard): if attributes don't follow the UTF-8 format
    /// - Err(TockloaderError::ProbeRsReadError): if reading fails
    pub(crate) fn read_system_attributes_probe(
        board_core: &mut Core,
    ) -> Result<Self, TockloaderError> {
        let mut result = SystemAttributes::new();
        // System attributes start at 0x600 and up to 0x9FF. See:
        // https://book.tockos.org/doc/memory_layout#flash-1
        let address = 0x600;
        // Each attribute is 64 bytes exactly, and there are 16 slots
        let mut buf = [0u8; 64 * 16];

        board_core.read(address, &mut buf)?;

        let mut data = buf.chunks(64);

        for current_slot in 0..data.len() {
            let slot_data = match data.next() {
                Some(data) => data,
                None => break,
            };

            // If the attribute chunk was successfully decoded, assign its value
            // to the corresponding field in `result` based on the index:
            // - 0 = board name,
            // - 1 = architecture,
            // - 2 = application start address (parsed from hex string),
            // - 3 = boot hash, _ = invalid or missing data is skipped.
            // NOTE: this can also be done by looping directly through the key attributes.
            if let Some(decoded_attributes) = decode_attribute(slot_data) {
                match current_slot {
                    0 => {
                        result.board = Some(decoded_attributes.value.to_string());
                    }
                    1 => {
                        result.arch = Some(decoded_attributes.value.to_string());
                    }
                    2 => {
                        // Parse hex string like "0x40000" into actual u64 value
                        result.appaddr = Some(
                            u64::from_str_radix(
                                decoded_attributes
                                    .value
                                    .to_string()
                                    .trim_start_matches("0x"),
                                16,
                            )
                            .map_err(|e| {
                                TockError::AttributeParsing(AttributeParseError::InvalidNumber(e))
                            })?,
                        );
                    }
                    3 => {
                        result.boothash = Some(decoded_attributes.value.to_string());
                    }
                    _ => {}
                }
            } else {
                continue;
            }
        }

        // TODO(eva-cosma): separate kernel attributes from kernel flags.

        let address = 0x40E;
        let mut buf = [0u8; 8];

        board_core.read_8(address, &mut buf)?;

        let string = String::from_utf8(buf.to_vec())
            .map_err(|e| TockError::AttributeParsing(AttributeParseError::InvalidString(e)))?;

        let string = string.trim_matches(char::from(0));

        result.bootloader_version = Some(string.to_owned());

        // The 100 bytes prior to the application start address are reserved for the kernel attributes and flags
        let mut kernel_attr_binary = [0u8; 100];
        let kernel_attr_addr = result
            .appaddr
            .ok_or(TockError::MissingAttribute("appaddr".to_owned()))?
            - 100;
        board_core.read(kernel_attr_addr, &mut kernel_attr_binary)?;

        let sentinel = bytes_to_string(&kernel_attr_binary[96..100]);
        let kernel_version = LittleEndian::read_uint(&kernel_attr_binary[95..96], 1);

        let app_memory_len = LittleEndian::read_u32(&kernel_attr_binary[84..92]);
        let app_memory_start = LittleEndian::read_u32(&kernel_attr_binary[80..84]);

        let kernel_binary_start = LittleEndian::read_u32(&kernel_attr_binary[68..72]);
        let kernel_binary_len = LittleEndian::read_u32(&kernel_attr_binary[72..76]);

        result.sentinel = Some(sentinel);
        result.kernel_version = Some(kernel_version);
        result.app_mem_start = Some(app_memory_start);
        result.app_mem_len = Some(app_memory_len);
        result.kernel_bin_start = Some(kernel_binary_start);
        result.kernel_bin_len = Some(kernel_binary_len);

        Ok(result)
    }

    /// Read system attributes using a serial connection. A bootloader must be
    /// present on this board for this function to work properly.
    ///
    /// # Parameters
    /// - `port`: Serial access, obtained from a
    ///   [SerialConnection](crate::connection::SerialConnection)
    ///
    /// # Returns
    /// - Ok(result): if attributes were read successfully
    /// - Err(TockloaderError::MisconfiguredBoard): if no start address is found or valid
    /// - Err(TockloaderError::MisconfiguredBoard): if attributes don't follow the UTF-8 format
    /// - Err(TockloaderError::SerialReadError): if reading fails
    pub(crate) async fn read_system_attributes_serial(
        port: &mut SerialStream,
    ) -> Result<Self, TockloaderError> {
        let mut result = SystemAttributes::new();

        // System attributes start at 0x600 and up to 0x9FF. See:
        // https://book.tockos.org/doc/memory_layout#flash-1
        let mut pkt = (0x600_u32).to_le_bytes().to_vec();
        // Each attribute is 64 bytes exactly, and there are 16 slots
        let length = (1024_u16).to_le_bytes().to_vec();
        for i in length {
            pkt.push(i);
        }

        // Read the kernel attributes
        let (_, buf) = issue_command(
            port,
            Command::ReadRange,
            pkt,
            true,
            64 * 16,
            Response::ReadRange,
        )
        .await?;

        let mut data = buf.chunks(64);

        for current_slot in 0..data.len() {
            let slot_data = match data.next() {
                Some(data) => data,
                None => break,
            };

            if let Some(decoded_attributes) = decode_attribute(slot_data) {
                match current_slot {
                    0 => {
                        result.board = Some(decoded_attributes.value.to_string());
                    }
                    1 => {
                        result.arch = Some(decoded_attributes.value.to_string());
                    }
                    2 => {
                        // Parse hex string like "0x40000" into actual u64 value
                        result.appaddr = Some(
                            u64::from_str_radix(
                                decoded_attributes
                                    .value
                                    .to_string()
                                    .trim_start_matches("0x"),
                                16,
                            )
                            .map_err(|e| {
                                TockError::AttributeParsing(AttributeParseError::InvalidNumber(e))
                            })?,
                        );
                    }
                    3 => {
                        result.boothash = Some(decoded_attributes.value.to_string());
                    }
                    _ => {}
                }
            } else {
                continue;
            }
        }

        // TODO(eva-cosma): separate kernel attributes from kernel flags.

        let mut pkt = (0x40E_u32).to_le_bytes().to_vec();
        let length = (8_u16).to_le_bytes().to_vec();
        for i in length {
            pkt.push(i);
        }

        // Read bootloader version
        let (_, buf) =
            issue_command(port, Command::ReadRange, pkt, true, 8, Response::ReadRange).await?;

        let string = String::from_utf8(buf)
            .map_err(|e| TockError::AttributeParsing(AttributeParseError::InvalidString(e)))?;

        // Strip null bytes
        let string = string.trim_matches(char::from(0));
        result.bootloader_version = Some(string.to_owned());

        // The 100 bytes prior to the application start address are reserved
        // for the kernel attributes and flags
        let kernel_attr_addr = (result
            .appaddr
            .ok_or(TockError::MissingAttribute("appaddr".to_owned()))?
            - 100) as u32;
        let mut pkt = kernel_attr_addr.to_le_bytes().to_vec();
        let length = (100_u16).to_le_bytes().to_vec();
        for i in length {
            pkt.push(i);
        }

        // Read kernel flags
        let (_, kernel_attr_binary) = issue_command(
            port,
            Command::ReadRange,
            pkt,
            true,
            100,
            Response::ReadRange,
        )
        .await?;

        let sentinel = bytes_to_string(&kernel_attr_binary[96..100]);
        let kernel_version = LittleEndian::read_uint(&kernel_attr_binary[95..96], 1);

        let app_memory_len = LittleEndian::read_u32(&kernel_attr_binary[84..92]);
        let app_memory_start = LittleEndian::read_u32(&kernel_attr_binary[80..84]);

        let kernel_binary_start = LittleEndian::read_u32(&kernel_attr_binary[68..72]);
        let kernel_binary_len = LittleEndian::read_u32(&kernel_attr_binary[72..76]);

        result.sentinel = Some(sentinel);
        result.kernel_version = Some(kernel_version);
        result.app_mem_start = Some(app_memory_start);
        result.app_mem_len = Some(app_memory_len);
        result.kernel_bin_start = Some(kernel_binary_start);
        result.kernel_bin_len = Some(kernel_binary_len);

        Ok(result)
    }
}
