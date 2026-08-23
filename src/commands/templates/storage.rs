use anyhow::{Context, Result};

// ZIP constants
const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const CD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];

pub struct ZipEntry {
    pub filename: String,
    pub offset: u64,
    pub compressed_size: u64,
    pub method: u16,
    pub local_header_size: u32,
}

pub fn decode_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

pub fn decode_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

pub fn decode_u64(data: &[u8], offset: usize) -> u64 {
    if offset + 8 > data.len() {
        return 0;
    }
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
}

pub fn parse_zip64_extra(data: &[u8], comp_size: &mut u64, local_offset: &mut u64) {
    let mut pos = 0;
    while pos + 4 <= data.len() {
        let header_id = u16::from_le_bytes([data[pos], data[pos + 1]]);
        let data_size = u16::from_le_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if pos + data_size > data.len() {
            break;
        }

        if header_id == 0x0001 {
            // ZIP64 extended information extra field
            let mut field_pos = pos;
            if *comp_size == 0xFFFFFFFF && field_pos + 8 <= pos + data_size {
                *comp_size = decode_u64(data, field_pos);
                field_pos += 8;
            }
            if *local_offset == 0xFFFFFFFF && field_pos + 8 <= pos + data_size {
                *local_offset = decode_u64(data, field_pos);
            }
            break;
        }

        pos += data_size;
    }
}

pub fn find_zip_entries(data: &[u8], file_size: u64) -> Result<Vec<ZipEntry>> {
    // Find EOCD signature
    let eocd_pos = data
        .windows(4)
        .rposition(|w| w == EOCD_SIGNATURE)
        .context("Invalid ZIP: EOCD not found")?;

    // Check for ZIP64 EOCD
    let mut cd_start_offset = decode_u32(data, eocd_pos + 16) as u64;
    let mut total_entries = decode_u16(data, eocd_pos + 10) as u64;

    if total_entries == 0xFFFF || cd_start_offset == 0xFFFFFFFF {
        // Look for ZIP64 EOCD locator
        if eocd_pos >= 20 {
            let zip64_locator_pos = eocd_pos - 20;
            if zip64_locator_pos + 20 <= data.len()
                && data[zip64_locator_pos..zip64_locator_pos + 4] == [0x50, 0x4b, 0x06, 0x07]
            {
                let zip64_eocd_offset = decode_u64(data, zip64_locator_pos + 8) as usize;
                let buffer_start_abs = file_size as usize - data.len();
                let zip64_eocd_pos = zip64_eocd_offset.saturating_sub(buffer_start_abs);

                if zip64_eocd_pos + 56 <= data.len()
                    && data[zip64_eocd_pos..zip64_eocd_pos + 4] == [0x50, 0x4b, 0x06, 0x06]
                {
                    total_entries = decode_u64(data, zip64_eocd_pos + 24);
                    cd_start_offset = decode_u64(data, zip64_eocd_pos + 48);
                }
            }
        }
    }

    let buffer_start_abs = file_size as usize - data.len();
    let mut current_pos = cd_start_offset.saturating_sub(buffer_start_abs as u64) as usize;

    let mut entries = Vec::new();

    for _ in 0..total_entries {
        if current_pos + 46 > data.len() {
            break;
        }
        if data[current_pos..current_pos + 4] != CD_SIGNATURE {
            break;
        }

        let method = decode_u16(data, current_pos + 10);
        let mut comp_size = decode_u32(data, current_pos + 20) as u64;
        let name_len = decode_u16(data, current_pos + 28) as usize;
        let extra_len = decode_u16(data, current_pos + 30) as usize;
        let comm_len = decode_u16(data, current_pos + 32) as usize;
        let mut local_offset = decode_u32(data, current_pos + 42) as u64;

        let full_record_len = 46 + name_len + extra_len + comm_len;
        if current_pos + full_record_len > data.len() {
            break;
        }

        let name_bytes = &data[current_pos + 46..current_pos + 46 + name_len];
        let filename = String::from_utf8_lossy(name_bytes).to_string();

        // Parse ZIP64 extra field if present
        if comp_size == 0xFFFFFFFF || local_offset == 0xFFFFFFFF {
            let extra_start = current_pos + 46 + name_len;
            let extra_end = extra_start + extra_len;
            if extra_end <= data.len() {
                parse_zip64_extra(&data[extra_start..extra_end], &mut comp_size, &mut local_offset);
            }
        }

        // Local file header minimum size: 30 + name_len (extra field read separately after download)
        let local_header_size = 30 + name_len as u32;

        entries.push(ZipEntry {
            filename,
            offset: local_offset,
            compressed_size: comp_size,
            method,
            local_header_size,
        });

        current_pos += full_record_len;
    }

    Ok(entries)
}
