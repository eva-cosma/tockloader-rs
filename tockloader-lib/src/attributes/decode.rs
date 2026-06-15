// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright OXIDOS AUTOMOTIVE 2024.

/// Attributes are key-value pairs that describe hardware configuration, stored
/// in a fixed 64-byte format:
///
/// 1. Bytes 0–7: UTF-8 key string with null-byte padding for shorter strings
/// 2. Byte 8: Value length (Must be between 1 and 55)
/// 3. Bytes 9–63: UTF-8 value string. Null-padded to length.
///
/// See also <https://book.tockos.org/doc/kernel_attributes.html?highlight=attributes#header-format>
#[derive(Debug)]
pub struct DecodedAttribute {
    pub key: String,
    pub value: String,
}

impl DecodedAttribute {
    pub(crate) fn new(decoded_key: String, decoded_value: String) -> DecodedAttribute {
        DecodedAttribute {
            key: decoded_key,
            value: decoded_value,
        }
    }
}

/// Internal function used to decode the raw data into a [DecodedAttribute].
///
/// # Params
/// - `step` - byte array of at least 64 bytes.
///
/// # Returns
/// - `None` for an invalid property (invalid value length or invalid utf-8
///   data).
/// - `Some(_)` otherwise
pub(crate) fn decode_attribute(step: &[u8]) -> Option<DecodedAttribute> {
    let raw_key = &step[0..8];

    let decoder_key = utf8_decode::Decoder::new(raw_key.iter().cloned());

    let mut key = String::new();
    for n in decoder_key {
        key.push(n.expect("Error getting key for attributes."));
    }

    key = key.trim_end_matches('\0').to_string();
    let vlen = step[8];

    if vlen > 55 || vlen == 0 {
        return None;
    }
    let raw_value = &step[9..(9 + vlen as usize)];
    let decoder_value = utf8_decode::Decoder::new(raw_value.iter().cloned());

    let mut value = String::new();

    for n in decoder_value {
        value.push(n.expect("Error getting key for attributes."));
    }

    value = value.trim_end_matches('\0').to_string();
    Some(DecodedAttribute::new(key, value))
}

// TODO(eva-cosma) replace this function with std::str::from_utf8(...). It
// does the same thing.

/// Transform a byte-slice into a String.
///
/// # Panics
///
/// This code panics if the given bytes are not utf-8 representable
pub(crate) fn bytes_to_string(raw: &[u8]) -> String {
    let decoder = utf8_decode::Decoder::new(raw.iter().cloned());

    let mut string = String::new();
    for n in decoder {
        string.push(n.expect("Error getting key for attributes."));
    }
    string
}
