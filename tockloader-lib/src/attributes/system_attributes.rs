// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright OXIDOS AUTOMOTIVE 2024.

use byteorder::{ByteOrder, LittleEndian};

use crate::errors::{AttributeParseError, TockError, TockloaderError};
use crate::IO;

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

    /// Read system attributes using a generalized connection. A bootloader must be
    /// present on this board for this function to work properly.
    ///
    /// # Parameters
    /// - `conn` : Either a SerialConnection or a ProbeRSConnection
    ///
    /// # Returns
    /// - Ok(result): if attributes were read successfully
    /// - Err(TockloaderError::MisconfiguredBoard): if no start address is found or valid
    /// - Err(TockloaderError::MisconfiguredBoard): if attributes don't follow the UTF-8 format
    /// - Err(TockloaderError::ProbeRsReadError): if reading fails on ProbeRS
    /// - Err(TockloaderError::SerialReadError): if reading fails on Serial
    pub(crate) async fn read_system_attributes(
        conn: &mut dyn IO,
    ) -> Result<SystemAttributes, TockloaderError> {
        let mut result = SystemAttributes::new();
        // System attributes start at 0x600 and up to 0x9FF. See:
        // https://book.tockos.org/doc/memory_layout#flash-1
        let address = 0x600;

        let buf = conn.read(address, 64 * 16).await?;

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

        let buf = conn.read(address, 8).await?;

        let string = String::from_utf8(buf.to_vec())
            .map_err(|e| TockError::AttributeParsing(AttributeParseError::InvalidString(e)))?;

        let string = string.trim_matches(char::from(0));

        result.bootloader_version = Some(string.to_owned());

        // The 100 bytes prior to the application start address are reserved for the kernel attributes and flags
        let kernel_attr_addr = result
            .appaddr
            .ok_or(TockError::MissingAttribute("appaddr".to_owned()))?
            - 100;
        let kernel_attr_binary = conn.read(kernel_attr_addr, 100).await?;

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
