//! VDI 안의 MBR·GPT 파티션 테이블을 읽기 전용으로 검색한다.
//!
//! 이 모듈은 파티션 메타데이터만 읽고 저장하지 않는다. 모든 LBA 계산은 checked
//! arithmetic과 디스크 용량 검증을 거치며, 손상된 테이블은 게스트 파일시스템
//! 계층으로 전달하지 않는다.

use super::{GuestFileSystem, PartitionTableKind, VdiPartition, VirtualDiskError};

const MBR_SIGNATURE: [u8; 2] = [0x55, 0xaa];
const MBR_PARTITION_OFFSET: usize = 446;
const MBR_ENTRY_SIZE: usize = 16;
const MBR_PARTITION_COUNT: usize = 4;
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
const GPT_HEADER_MIN_SIZE: u32 = 92;
const GPT_ENTRY_MIN_SIZE: u32 = 128;
const GPT_ENTRY_MAX_SIZE: u32 = 4096;
const MAX_GPT_ARRAY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LOGICAL_PARTITIONS: usize = 128;
const TYPE_PROTECTIVE_MBR: u8 = 0xee;
const TYPE_EXTENDED_MBR: [u8; 3] = [0x05, 0x0f, 0x85];
const GPT_TYPE_MICROSOFT_RESERVED: [u8; 16] = [
    0x16, 0xe3, 0xc9, 0xe3, 0x5c, 0x0b, 0xb8, 0x4d, 0x81, 0x7d, 0xf9, 0x2d, 0xf0, 0x02,
    0x15, 0xae,
];
const BITLOCKER_BOOT_MARKER: &[u8; 8] = b"-FVE-FS-";

/// 파티션 검색에 필요한 읽기 전용 디스크 경계.
pub trait PartitionSource: Send {
    fn disk_size_bytes(&self) -> u64;
    fn sector_size(&self) -> u32;
    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VirtualDiskError>;
}

impl PartitionSource for super::vdi::VdiReader {
    fn disk_size_bytes(&self) -> u64 {
        self.image().disk_size_bytes
    }

    fn sector_size(&self) -> u32 {
        self.header().sector_size
    }

    fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VirtualDiskError> {
        super::vdi::VdiReader::read_at(self, offset, buffer)
    }
}

/// MBR 또는 GPT의 유효한 파티션을 검색한다.
pub fn discover_partitions<S: PartitionSource>(
    source: &mut S,
) -> Result<Vec<VdiPartition>, VirtualDiskError> {
    let sector_size = source.sector_size();
    if !matches!(sector_size, 512 | 4096) {
        return Err(VirtualDiskError::UnsupportedFormat {
            kind: super::UnsupportedFormatKind::SectorSize,
            detail: format!("파티션 검색은 512 또는 4096바이트 섹터만 지원합니다: {sector_size}"),
        });
    }
    if !source.disk_size_bytes().is_multiple_of(sector_size as u64) {
        return Err(corrupt(
            "디스크 용량이 논리 섹터 크기로 나누어지지 않습니다",
        ));
    }

    let total_sectors = source.disk_size_bytes() / sector_size as u64;
    if total_sectors < 2 {
        return Err(corrupt("파티션 테이블을 읽기에는 디스크가 너무 작습니다"));
    }

    let mbr = read_sector(source, 0)?;
    validate_mbr_signature(&mbr)?;
    let mut entries = Vec::with_capacity(MBR_PARTITION_COUNT);
    let mut protective = false;
    let mut non_protective = false;

    for index in 0..MBR_PARTITION_COUNT {
        let entry = MbrEntry::parse(&mbr, index)?;
        if entry.is_empty() {
            continue;
        }
        if entry.partition_type == TYPE_PROTECTIVE_MBR {
            // GPT 보호 MBR은 32비트 MBR 범위를 넘어서는 디스크를 보호하기 위해
            // 섹터 수를 `0xffff_ffff`로 기록할 수 있다. 실제 파티션 범위는 GPT
            // 헤더가 소유하므로, 여기서는 시작 LBA만 현재 디스크 안인지 확인한다.
            if entry.start_lba == 0 || entry.start_lba as u64 >= total_sectors {
                return Err(corrupt("보호 MBR 시작 LBA가 디스크 범위를 벗어납니다"));
            }
            protective = true;
        } else {
            validate_mbr_range(
                entry.start_lba as u64,
                entry.sector_count as u64,
                total_sectors,
            )?;
            non_protective = true;
        }
        entries.push((index as u32 + 1, entry));
    }

    if protective {
        if non_protective {
            return Err(corrupt(
                "보호 MBR과 일반 MBR 항목이 함께 있는 hybrid 디스크는 지원하지 않습니다",
            ));
        }
        return parse_gpt(source, total_sectors, sector_size);
    }

    parse_mbr(source, total_sectors, entries)
}

fn parse_mbr<S: PartitionSource>(
    source: &mut S,
    total_sectors: u64,
    entries: Vec<(u32, MbrEntry)>,
) -> Result<Vec<VdiPartition>, VirtualDiskError> {
    let mut partitions = Vec::new();
    let mut extended: Option<MbrEntry> = None;

    for (number, entry) in entries {
        if TYPE_EXTENDED_MBR.contains(&entry.partition_type) {
            if extended.replace(entry).is_some() {
                return Err(corrupt("확장 MBR 항목이 여러 개입니다"));
            }
            continue;
        }
        partitions.push(to_partition(
            source,
            number,
            PartitionTableKind::Mbr,
            &entry,
        )?);
    }

    if let Some(extended) = extended {
        partitions.extend(parse_extended_partitions(
            source,
            total_sectors,
            extended.start_lba as u64,
            extended.sector_count as u64,
        )?);
    }
    Ok(partitions)
}

fn parse_extended_partitions<S: PartitionSource>(
    source: &mut S,
    total_sectors: u64,
    base_lba: u64,
    sector_count: u64,
) -> Result<Vec<VdiPartition>, VirtualDiskError> {
    let extended_end = base_lba
        .checked_add(sector_count)
        .ok_or_else(|| corrupt("확장 MBR 범위가 오버플로됩니다"))?;
    validate_mbr_range(base_lba, sector_count, total_sectors)?;

    let mut result = Vec::new();
    let mut current_lba = base_lba;
    let mut seen = std::collections::HashSet::new();

    for number in 5..(5 + MAX_LOGICAL_PARTITIONS as u32) {
        if !seen.insert(current_lba) {
            return Err(corrupt("확장 MBR 연결이 순환합니다"));
        }
        if current_lba < base_lba || current_lba >= extended_end {
            return Err(corrupt("확장 MBR 연결이 컨테이너 범위를 벗어납니다"));
        }

        let ebr = read_sector_at(source, current_lba)?;
        validate_mbr_signature(&ebr)?;
        let logical = MbrEntry::parse(&ebr, 0)?;
        let link = MbrEntry::parse(&ebr, 1)?;

        if !logical.is_empty() {
            if TYPE_EXTENDED_MBR.contains(&logical.partition_type) {
                return Err(corrupt("확장 MBR의 논리 항목이 다시 확장 유형입니다"));
            }
            let start = current_lba
                .checked_add(logical.start_lba as u64)
                .ok_or_else(|| corrupt("논리 파티션 시작 LBA가 오버플로됩니다"))?;
            validate_range_in(start, logical.sector_count as u64, base_lba, extended_end)?;
            validate_mbr_range(start, logical.sector_count as u64, total_sectors)?;
            result.push(to_partition_at(
                source,
                number,
                PartitionTableKind::Mbr,
                start,
                logical.sector_count as u64,
                None,
            )?);
        }

        if link.is_empty() {
            break;
        }
        if !TYPE_EXTENDED_MBR.contains(&link.partition_type) {
            return Err(corrupt("확장 MBR 연결 항목의 유형이 잘못되었습니다"));
        }
        let next = base_lba
            .checked_add(link.start_lba as u64)
            .ok_or_else(|| corrupt("다음 EBR 시작 LBA가 오버플로됩니다"))?;
        validate_range_in(next, link.sector_count as u64, base_lba, extended_end)?;
        current_lba = next;
    }

    if seen.len() == MAX_LOGICAL_PARTITIONS {
        return Err(corrupt("확장 MBR 논리 파티션 수가 안전 한도를 초과합니다"));
    }
    Ok(result)
}

fn parse_gpt<S: PartitionSource>(
    source: &mut S,
    total_sectors: u64,
    sector_size: u32,
) -> Result<Vec<VdiPartition>, VirtualDiskError> {
    let header = read_sector_at(source, 1)?;
    if header.get(0..8) != Some(GPT_SIGNATURE.as_slice()) {
        return Err(corrupt("보호 MBR 뒤에 GPT 헤더가 없습니다"));
    }

    let header_size = le_u32(&header, 12)?;
    if !(GPT_HEADER_MIN_SIZE..=sector_size).contains(&header_size) {
        return Err(corrupt(format!(
            "GPT 헤더 크기가 유효하지 않습니다: {header_size}"
        )));
    }
    let stored_header_crc = le_u32(&header, 16)?;
    let mut header_for_crc = header[..header_size as usize].to_vec();
    header_for_crc[16..20].fill(0);
    if crc32(&header_for_crc) != stored_header_crc {
        return Err(corrupt("GPT 주 헤더 CRC가 일치하지 않습니다"));
    }

    let current_lba = le_u64(&header, 24)?;
    let backup_lba = le_u64(&header, 32)?;
    let first_usable = le_u64(&header, 40)?;
    let last_usable = le_u64(&header, 48)?;
    let entries_lba = le_u64(&header, 72)?;
    let entry_count = le_u32(&header, 80)?;
    let entry_size = le_u32(&header, 84)?;
    let stored_array_crc = le_u32(&header, 88)?;

    if current_lba != 1
        || backup_lba >= total_sectors
        || first_usable < 2
        || first_usable > last_usable
        || last_usable >= backup_lba
    {
        return Err(corrupt("GPT 헤더의 LBA 범위가 유효하지 않습니다"));
    }
    if entry_count == 0
        || !(GPT_ENTRY_MIN_SIZE..=GPT_ENTRY_MAX_SIZE).contains(&entry_size)
        || entry_size % 8 != 0
    {
        return Err(corrupt(
            "GPT 파티션 배열의 항목 크기 또는 개수가 유효하지 않습니다",
        ));
    }

    let array_bytes = (entry_count as u64)
        .checked_mul(entry_size as u64)
        .ok_or_else(|| corrupt("GPT 파티션 배열 크기가 오버플로됩니다"))?;
    if array_bytes > MAX_GPT_ARRAY_BYTES {
        return Err(corrupt("GPT 파티션 배열이 안전한 메모리 한도를 초과합니다"));
    }
    let array_end = entries_lba
        .checked_mul(sector_size as u64)
        .and_then(|offset| offset.checked_add(array_bytes))
        .ok_or_else(|| corrupt("GPT 파티션 배열 오프셋이 오버플로됩니다"))?;
    let first_usable_offset = first_usable
        .checked_mul(sector_size as u64)
        .ok_or_else(|| corrupt("GPT 사용 가능 영역 오프셋이 오버플로됩니다"))?;
    if entries_lba < 2
        || entries_lba >= total_sectors
        || array_end > first_usable_offset
        || array_end > total_sectors * sector_size as u64
    {
        return Err(corrupt("GPT 파티션 배열이 디스크 범위를 벗어납니다"));
    }

    let array_offset = entries_lba * sector_size as u64;
    let mut array = vec![0u8; array_bytes as usize];
    read_exact(source, array_offset, &mut array)?;
    if crc32(&array) != stored_array_crc {
        return Err(corrupt("GPT 파티션 배열 CRC가 일치하지 않습니다"));
    }
    validate_backup_gpt(
        source,
        total_sectors,
        sector_size,
        backup_lba,
        &GptLayout {
            first_usable,
            last_usable,
            entry_count,
            entry_size,
            expected_array_crc: stored_array_crc,
        },
    )?;

    let entry_size = entry_size as usize;
    let mut result = Vec::new();
    for index in 0..entry_count as usize {
        let offset = index
            .checked_mul(entry_size)
            .ok_or_else(|| corrupt("GPT 파티션 항목 오프셋이 오버플로됩니다"))?;
        let entry = &array[offset..offset + entry_size];
        if entry[..16].iter().all(|byte| *byte == 0) {
            continue;
        }
        let start_lba = le_u64(entry, 32)?;
        let end_lba = le_u64(entry, 40)?;
        if start_lba > end_lba || start_lba < first_usable || end_lba > last_usable {
            return Err(corrupt(format!(
                "GPT 파티션 {index}의 범위가 유효하지 않습니다"
            )));
        }
        let sector_count = end_lba
            .checked_sub(start_lba)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| corrupt("GPT 파티션 섹터 수가 오버플로됩니다"))?;
        result.push(to_partition_at(
            source,
            u32::try_from(index + 1)
                .map_err(|_| corrupt("GPT 파티션 번호가 u32 범위를 초과합니다"))?,
            PartitionTableKind::Gpt,
            start_lba,
            sector_count,
            (entry.get(..16) == Some(GPT_TYPE_MICROSOFT_RESERVED.as_slice()))
                .then_some(GuestFileSystem::MicrosoftReserved),
        )?);
    }
    Ok(result)
}

fn validate_backup_gpt<S: PartitionSource>(
    source: &mut S,
    total_sectors: u64,
    sector_size: u32,
    backup_lba: u64,
    layout: &GptLayout,
) -> Result<(), VirtualDiskError> {
    if backup_lba != total_sectors - 1 {
        return Err(corrupt(
            "GPT 백업 헤더가 디스크 마지막 섹터에 있지 않습니다",
        ));
    }
    let header = read_sector_at(source, backup_lba)?;
    if header.get(0..8) != Some(GPT_SIGNATURE.as_slice()) {
        return Err(corrupt("GPT 백업 헤더 서명이 없습니다"));
    }
    let header_size = le_u32(&header, 12)?;
    if !(GPT_HEADER_MIN_SIZE..=sector_size).contains(&header_size) {
        return Err(corrupt(format!(
            "GPT 백업 헤더 크기가 유효하지 않습니다: {header_size}"
        )));
    }
    let stored_header_crc = le_u32(&header, 16)?;
    let mut header_for_crc = header[..header_size as usize].to_vec();
    header_for_crc[16..20].fill(0);
    if crc32(&header_for_crc) != stored_header_crc {
        return Err(corrupt("GPT 백업 헤더 CRC가 일치하지 않습니다"));
    }

    if le_u64(&header, 24)? != backup_lba
        || le_u64(&header, 32)? != 1
        || le_u64(&header, 40)? != layout.first_usable
        || le_u64(&header, 48)? != layout.last_usable
        || le_u32(&header, 80)? != layout.entry_count
        || le_u32(&header, 84)? != layout.entry_size
    {
        return Err(corrupt("GPT 백업 헤더가 주 헤더와 일치하지 않습니다"));
    }

    let entries_lba = le_u64(&header, 72)?;
    let array_bytes = (layout.entry_count as u64)
        .checked_mul(layout.entry_size as u64)
        .ok_or_else(|| corrupt("GPT 백업 배열 크기가 오버플로됩니다"))?;
    let array_offset = entries_lba
        .checked_mul(sector_size as u64)
        .ok_or_else(|| corrupt("GPT 백업 배열 오프셋이 오버플로됩니다"))?;
    let array_end = array_offset
        .checked_add(array_bytes)
        .ok_or_else(|| corrupt("GPT 백업 배열 끝 오프셋이 오버플로됩니다"))?;
    let backup_offset = backup_lba
        .checked_mul(sector_size as u64)
        .ok_or_else(|| corrupt("GPT 백업 헤더 오프셋이 오버플로됩니다"))?;
    if entries_lba >= total_sectors || array_end > backup_offset {
        return Err(corrupt("GPT 백업 배열이 디스크 범위를 벗어납니다"));
    }

    let mut array = vec![0u8; array_bytes as usize];
    read_exact(source, array_offset, &mut array)?;
    if crc32(&array) != layout.expected_array_crc
        || le_u32(&header, 88)? != layout.expected_array_crc
    {
        return Err(corrupt("GPT 백업 파티션 배열 CRC가 일치하지 않습니다"));
    }
    Ok(())
}

fn read_sector<S: PartitionSource>(source: &mut S, lba: u64) -> Result<Vec<u8>, VirtualDiskError> {
    read_sector_at(source, lba)
}

fn read_sector_at<S: PartitionSource>(
    source: &mut S,
    lba: u64,
) -> Result<Vec<u8>, VirtualDiskError> {
    let offset = lba
        .checked_mul(source.sector_size() as u64)
        .ok_or_else(|| corrupt("파티션 섹터 오프셋이 오버플로됩니다"))?;
    let mut sector = vec![0u8; source.sector_size() as usize];
    read_exact(source, offset, &mut sector)?;
    Ok(sector)
}

fn read_exact<S: PartitionSource>(
    source: &mut S,
    offset: u64,
    buffer: &mut [u8],
) -> Result<(), VirtualDiskError> {
    let read = source.read_at(offset, buffer)?;
    if read != buffer.len() {
        return Err(corrupt(format!(
            "파티션 데이터가 완전히 읽히지 않았습니다: {read}/{}",
            buffer.len()
        )));
    }
    Ok(())
}

fn validate_mbr_signature(sector: &[u8]) -> Result<(), VirtualDiskError> {
    if sector.get(510..512) != Some(MBR_SIGNATURE.as_slice()) {
        return Err(corrupt("MBR 서명 0x55AA가 없습니다"));
    }
    Ok(())
}

fn validate_mbr_range(start: u64, count: u64, total: u64) -> Result<(), VirtualDiskError> {
    if start == 0 || count == 0 {
        return Err(corrupt("MBR 파티션의 시작 LBA 또는 섹터 수가 0입니다"));
    }
    let end = start
        .checked_add(count)
        .ok_or_else(|| corrupt("MBR 파티션 범위가 오버플로됩니다"))?;
    if end > total {
        return Err(corrupt("MBR 파티션이 디스크 범위를 벗어납니다"));
    }
    Ok(())
}

fn validate_range_in(
    start: u64,
    count: u64,
    lower: u64,
    upper: u64,
) -> Result<(), VirtualDiskError> {
    validate_mbr_range(start, count, upper)?;
    let end = start + count;
    if start < lower || end > upper {
        return Err(corrupt("논리 파티션이 확장 MBR 범위를 벗어납니다"));
    }
    Ok(())
}

fn to_partition<S: PartitionSource>(
    source: &mut S,
    number: u32,
    table: PartitionTableKind,
    entry: &MbrEntry,
) -> Result<VdiPartition, VirtualDiskError> {
    to_partition_at(
        source,
        number,
        table,
        entry.start_lba as u64,
        entry.sector_count as u64,
        None,
    )
}

fn to_partition_at<S: PartitionSource>(
    source: &mut S,
    number: u32,
    table: PartitionTableKind,
    start_lba: u64,
    sector_count: u64,
    filesystem_hint: Option<GuestFileSystem>,
) -> Result<VdiPartition, VirtualDiskError> {
    Ok(VdiPartition {
        number,
        table,
        start_lba,
        sector_count,
        filesystem: detect_filesystem(source, start_lba, filesystem_hint)?,
    })
}

fn detect_filesystem<S: PartitionSource>(
    source: &mut S,
    start_lba: u64,
    filesystem_hint: Option<GuestFileSystem>,
) -> Result<Option<GuestFileSystem>, VirtualDiskError> {
    let boot_sector = read_sector_at(source, start_lba)?;
    Ok(identify_filesystem(
        &boot_sector,
        source.sector_size(),
        filesystem_hint,
    ))
}

fn identify_filesystem(
    boot_sector: &[u8],
    sector_size: u32,
    filesystem_hint: Option<GuestFileSystem>,
) -> Option<GuestFileSystem> {
    if filesystem_hint == Some(GuestFileSystem::MicrosoftReserved) {
        return filesystem_hint;
    }
    if boot_sector.get(3..11) == Some(BITLOCKER_BOOT_MARKER.as_slice()) {
        return Some(GuestFileSystem::BitLocker);
    }
    let is_ntfs = boot_sector.get(3..11) == Some(b"NTFS    ")
        && le_u16(boot_sector, 11).ok()? == sector_size as u16
        && boot_sector.get(13).copied().is_some_and(|value| value > 0);
    is_ntfs.then_some(GuestFileSystem::ntfs_3_1())
}

struct MbrEntry {
    partition_type: u8,
    start_lba: u32,
    sector_count: u32,
}

struct GptLayout {
    first_usable: u64,
    last_usable: u64,
    entry_count: u32,
    entry_size: u32,
    expected_array_crc: u32,
}

impl MbrEntry {
    fn parse(sector: &[u8], index: usize) -> Result<Self, VirtualDiskError> {
        let offset = MBR_PARTITION_OFFSET
            .checked_add(index * MBR_ENTRY_SIZE)
            .ok_or_else(|| corrupt("MBR 항목 오프셋이 오버플로됩니다"))?;
        let entry = sector
            .get(offset..offset + MBR_ENTRY_SIZE)
            .ok_or_else(|| corrupt("MBR 파티션 항목이 잘렸습니다"))?;
        let partition_type = entry[4];
        let start_lba = u32::from_le_bytes(entry[8..12].try_into().expect("4-byte MBR field"));
        let sector_count = u32::from_le_bytes(entry[12..16].try_into().expect("4-byte MBR field"));
        if partition_type == 0 && (start_lba != 0 || sector_count != 0) {
            return Err(corrupt("비어 있지 않은 MBR 항목의 유형이 0입니다"));
        }
        if partition_type != 0 && sector_count == 0 {
            return Err(corrupt("MBR 파티션 항목의 섹터 수가 0입니다"));
        }
        Ok(Self {
            partition_type,
            start_lba,
            sector_count,
        })
    }

    fn is_empty(&self) -> bool {
        self.partition_type == 0
    }
}

fn le_u32(raw: &[u8], offset: usize) -> Result<u32, VirtualDiskError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or_else(|| corrupt("파티션 테이블 필드가 잘렸습니다"))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("4-byte partition field"),
    ))
}

fn le_u16(raw: &[u8], offset: usize) -> Result<u16, VirtualDiskError> {
    let bytes = raw
        .get(offset..offset + 2)
        .ok_or_else(|| corrupt("파티션 테이블 필드가 잘렸습니다"))?;
    Ok(u16::from_le_bytes(
        bytes.try_into().expect("2-byte partition field"),
    ))
}

fn le_u64(raw: &[u8], offset: usize) -> Result<u64, VirtualDiskError> {
    let bytes = raw
        .get(offset..offset + 8)
        .ok_or_else(|| corrupt("파티션 테이블 필드가 잘렸습니다"))?;
    Ok(u64::from_le_bytes(
        bytes.try_into().expect("8-byte partition field"),
    ))
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn corrupt(detail: impl Into<String>) -> VirtualDiskError {
    VirtualDiskError::CorruptImage(format!("파티션 테이블: {}", detail.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECTOR_SIZE: usize = 512;

    struct MemoryDisk {
        bytes: Vec<u8>,
    }

    impl MemoryDisk {
        fn new(sectors: usize) -> Self {
            Self {
                bytes: vec![0; sectors * SECTOR_SIZE],
            }
        }

        fn mbr_entry(&mut self, index: usize, partition_type: u8, start: u32, count: u32) {
            let offset = MBR_PARTITION_OFFSET + index * MBR_ENTRY_SIZE;
            self.bytes[offset + 4] = partition_type;
            self.bytes[offset + 8..offset + 12].copy_from_slice(&start.to_le_bytes());
            self.bytes[offset + 12..offset + 16].copy_from_slice(&count.to_le_bytes());
        }

        fn set_mbr_signature(&mut self) {
            self.bytes[510..512].copy_from_slice(&MBR_SIGNATURE);
        }

        fn refresh_gpt_crcs(&mut self, entry_count: usize) {
            let array_len = entry_count * 128;
            let array = self.bytes[2 * SECTOR_SIZE..2 * SECTOR_SIZE + array_len].to_vec();
            let array_crc = crc32(&array);
            self.bytes[SECTOR_SIZE + 88..SECTOR_SIZE + 92]
                .copy_from_slice(&array_crc.to_le_bytes());
            self.bytes[SECTOR_SIZE + 16..SECTOR_SIZE + 20].fill(0);
            let header_crc = crc32(&self.bytes[SECTOR_SIZE..SECTOR_SIZE + 92]);
            self.bytes[SECTOR_SIZE + 16..SECTOR_SIZE + 20]
                .copy_from_slice(&header_crc.to_le_bytes());

            let backup_lba = self.bytes.len() / SECTOR_SIZE - 1;
            let backup_array_lba = backup_lba - 1;
            let backup_array_offset = backup_array_lba * SECTOR_SIZE;
            self.bytes[backup_array_offset..backup_array_offset + array_len]
                .copy_from_slice(&array);
            let mut backup_header = self.bytes[SECTOR_SIZE..2 * SECTOR_SIZE].to_vec();
            backup_header[24..32].copy_from_slice(&(backup_lba as u64).to_le_bytes());
            backup_header[32..40].copy_from_slice(&1u64.to_le_bytes());
            backup_header[72..80].copy_from_slice(&(backup_array_lba as u64).to_le_bytes());
            backup_header[88..92].copy_from_slice(&array_crc.to_le_bytes());
            backup_header[16..20].fill(0);
            let backup_header_crc = crc32(&backup_header[..92]);
            backup_header[16..20].copy_from_slice(&backup_header_crc.to_le_bytes());
            let backup_header_offset = backup_lba * SECTOR_SIZE;
            self.bytes[backup_header_offset..backup_header_offset + SECTOR_SIZE]
                .copy_from_slice(&backup_header);
        }
    }

    impl PartitionSource for MemoryDisk {
        fn disk_size_bytes(&self) -> u64 {
            self.bytes.len() as u64
        }

        fn sector_size(&self) -> u32 {
            SECTOR_SIZE as u32
        }

        fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VirtualDiskError> {
            let end = offset
                .checked_add(buffer.len() as u64)
                .ok_or_else(|| corrupt("테스트 디스크 범위가 오버플로됩니다"))?;
            if end > self.bytes.len() as u64 {
                return Err(VirtualDiskError::BoundsViolation {
                    offset,
                    length: buffer.len() as u64,
                    capacity: self.bytes.len() as u64,
                });
            }
            buffer.copy_from_slice(&self.bytes[offset as usize..end as usize]);
            Ok(buffer.len())
        }
    }

    #[test]
    fn discovers_primary_mbr_partition() {
        let mut disk = MemoryDisk::new(4096);
        disk.set_mbr_signature();
        disk.mbr_entry(0, 0x07, 2048, 1024);

        let partitions = discover_partitions(&mut disk).unwrap();

        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].table, PartitionTableKind::Mbr);
        assert_eq!(partitions[0].start_lba, 2048);
        assert_eq!(partitions[0].sector_count, 1024);
        assert_eq!(partitions[0].filesystem, None);
    }

    #[test]
    fn detects_ntfs_3_1_from_partition_boot_sector() {
        let mut disk = MemoryDisk::new(4096);
        disk.set_mbr_signature();
        disk.mbr_entry(0, 0x07, 2048, 1024);
        let boot = 2048 * SECTOR_SIZE;
        disk.bytes[boot + 3..boot + 11].copy_from_slice(b"NTFS    ");
        disk.bytes[boot + 11..boot + 13].copy_from_slice(&(SECTOR_SIZE as u16).to_le_bytes());
        disk.bytes[boot + 13] = 8;

        let partitions = discover_partitions(&mut disk).unwrap();

        assert_eq!(partitions[0].filesystem, Some(GuestFileSystem::ntfs_3_1()));
    }

    #[test]
    fn classifies_bitlocker_and_microsoft_reserved_partitions_explicitly() {
        let mut bitlocker_boot = vec![0; SECTOR_SIZE];
        bitlocker_boot[3..11].copy_from_slice(BITLOCKER_BOOT_MARKER);
        assert_eq!(
            identify_filesystem(&bitlocker_boot, SECTOR_SIZE as u32, None),
            Some(GuestFileSystem::BitLocker)
        );

        let empty_boot = vec![0; SECTOR_SIZE];
        assert_eq!(
            identify_filesystem(
                &empty_boot,
                SECTOR_SIZE as u32,
                Some(GuestFileSystem::MicrosoftReserved),
            ),
            Some(GuestFileSystem::MicrosoftReserved)
        );
    }

    #[test]
    fn discovers_logical_mbr_partition_through_ebr() {
        let mut disk = MemoryDisk::new(4096);
        disk.set_mbr_signature();
        disk.mbr_entry(0, 0x05, 100, 1000);
        let ebr = 100 * SECTOR_SIZE;
        disk.bytes[ebr + 510..ebr + 512].copy_from_slice(&MBR_SIGNATURE);
        disk.bytes[ebr + 446 + 4] = 0x07;
        disk.bytes[ebr + 446 + 8..ebr + 446 + 12].copy_from_slice(&1u32.to_le_bytes());
        disk.bytes[ebr + 446 + 12..ebr + 446 + 16].copy_from_slice(&100u32.to_le_bytes());

        let partitions = discover_partitions(&mut disk).unwrap();

        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].number, 5);
        assert_eq!(partitions[0].start_lba, 101);
    }

    #[test]
    fn discovers_gpt_and_ignores_protective_mbr() {
        let mut disk = MemoryDisk::new(4096);
        disk.set_mbr_signature();
        disk.mbr_entry(0, TYPE_PROTECTIVE_MBR, 1, u32::MAX);
        disk.bytes[SECTOR_SIZE..SECTOR_SIZE + 8].copy_from_slice(GPT_SIGNATURE);
        disk.bytes[SECTOR_SIZE + 12..SECTOR_SIZE + 16].copy_from_slice(&92u32.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 24..SECTOR_SIZE + 32].copy_from_slice(&1u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 32..SECTOR_SIZE + 40].copy_from_slice(&4095u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 40..SECTOR_SIZE + 48].copy_from_slice(&34u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 48..SECTOR_SIZE + 56].copy_from_slice(&4062u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 72..SECTOR_SIZE + 80].copy_from_slice(&2u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 80..SECTOR_SIZE + 84].copy_from_slice(&4u32.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 84..SECTOR_SIZE + 88].copy_from_slice(&128u32.to_le_bytes());
        let entry = 2 * SECTOR_SIZE;
        disk.bytes[entry] = 1;
        disk.bytes[entry + 32..entry + 40].copy_from_slice(&34u64.to_le_bytes());
        disk.bytes[entry + 40..entry + 48].copy_from_slice(&100u64.to_le_bytes());
        disk.refresh_gpt_crcs(4);

        let partitions = discover_partitions(&mut disk).unwrap();

        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].table, PartitionTableKind::Gpt);
        assert_eq!(partitions[0].sector_count, 67);
    }

    #[test]
    fn rejects_mbr_partition_outside_disk() {
        let mut disk = MemoryDisk::new(4096);
        disk.set_mbr_signature();
        disk.mbr_entry(0, 0x07, 4000, 200);

        assert!(matches!(
            discover_partitions(&mut disk),
            Err(VirtualDiskError::CorruptImage(message)) if message.contains("범위")
        ));
    }

    #[test]
    fn rejects_gpt_header_crc_mismatch() {
        let mut disk = MemoryDisk::new(4096);
        disk.set_mbr_signature();
        disk.mbr_entry(0, TYPE_PROTECTIVE_MBR, 1, 4095);
        disk.bytes[SECTOR_SIZE..SECTOR_SIZE + 8].copy_from_slice(GPT_SIGNATURE);
        disk.bytes[SECTOR_SIZE + 12..SECTOR_SIZE + 16].copy_from_slice(&92u32.to_le_bytes());
        disk.refresh_gpt_crcs(4);
        disk.bytes[SECTOR_SIZE + 24] ^= 1;

        assert!(matches!(
            discover_partitions(&mut disk),
            Err(VirtualDiskError::CorruptImage(message)) if message.contains("GPT 주 헤더 CRC")
        ));
    }

    #[test]
    fn rejects_gpt_backup_header_crc_mismatch() {
        let mut disk = MemoryDisk::new(4096);
        disk.set_mbr_signature();
        disk.mbr_entry(0, TYPE_PROTECTIVE_MBR, 1, 4095);
        disk.bytes[SECTOR_SIZE..SECTOR_SIZE + 8].copy_from_slice(GPT_SIGNATURE);
        disk.bytes[SECTOR_SIZE + 12..SECTOR_SIZE + 16].copy_from_slice(&92u32.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 24..SECTOR_SIZE + 32].copy_from_slice(&1u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 32..SECTOR_SIZE + 40].copy_from_slice(&4095u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 40..SECTOR_SIZE + 48].copy_from_slice(&34u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 48..SECTOR_SIZE + 56].copy_from_slice(&4062u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 72..SECTOR_SIZE + 80].copy_from_slice(&2u64.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 80..SECTOR_SIZE + 84].copy_from_slice(&4u32.to_le_bytes());
        disk.bytes[SECTOR_SIZE + 84..SECTOR_SIZE + 88].copy_from_slice(&128u32.to_le_bytes());
        disk.refresh_gpt_crcs(4);
        let backup_header = (4095 * SECTOR_SIZE) + 16;
        disk.bytes[backup_header] ^= 1;

        assert!(matches!(
            discover_partitions(&mut disk),
            Err(VirtualDiskError::CorruptImage(message)) if message.contains("GPT 백업 헤더 CRC")
        ));
    }
}
