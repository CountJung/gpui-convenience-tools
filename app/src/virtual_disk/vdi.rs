//! VDI 1.1 read-only 컨테이너와 블록 맵 리더.
//!
//! 이 모듈은 `File::open`으로 원본을 읽기 전용으로 열고, VDI 가상 디스크의
//! 오프셋을 호스트 파일의 물리 블록으로 변환한다. 동적 이미지의 미할당 블록과
//! discarded 블록은 VDI 계약에 따라 0으로 반환한다. 차등 이미지·부모 체인은
//! VDE-004 범위 밖이므로 열지 않는다.

use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use super::{UnsupportedFormatKind, VdiImage, VdiImageType, VdiVersion, VirtualDiskError};

const HEADER_SIZE: usize = 512;
const SIGNATURE: u32 = 0xbeda_107f;
const IMAGE_TYPE_DYNAMIC: u32 = 1;
const IMAGE_TYPE_FIXED: u32 = 2;
const IMAGE_TYPE_DIFFERENCING: u32 = 4;
const BLOCK_UNALLOCATED: u32 = 0xffff_ffff;
const BLOCK_DISCARDED: u32 = 0xffff_fffe;
const SECTOR_SIZE: u32 = 512;
const MIN_HEADER_MAIN_SIZE: u32 = 0x180;
const MAX_BLOCK_MAP_BYTES: u64 = 256 * 1024 * 1024;

/// VDI 1.1 헤더에서 후속 블록 리더가 사용하는 값.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VdiHeader {
    pub version: VdiVersion,
    pub header_size: u32,
    pub image_type: VdiImageType,
    pub image_flags: u32,
    pub offset_bmap: u32,
    pub offset_data: u32,
    pub sector_size: u32,
    pub disk_size: u64,
    pub block_size: u32,
    pub block_extra: u32,
    pub blocks_in_image: u32,
    pub blocks_allocated: u32,
}

/// VDI를 열기 전에 적용할 안전 검사 설정.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VdiSafetyOptions {
    /// 실행 중 VM의 연결 디스크를 조회할 `VBoxManage` 경로.
    ///
    /// `GPUI_CONVENIENCE_TOOLS_VBOXMANAGE`에서 읽을 수 있으며, 설정하지 않으면
    /// 파일시스템 잠금 표식 검사만 수행한다. 명시된 경로의 조회가 실패하면
    /// 안전하게 열기를 거부한다.
    pub vboxmanage_path: Option<PathBuf>,
}

impl VdiSafetyOptions {
    pub fn from_environment() -> Self {
        Self {
            vboxmanage_path: env::var_os("GPUI_CONVENIENCE_TOOLS_VBOXMANAGE").map(PathBuf::from),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceSnapshot {
    file_len: u64,
    modified: SystemTime,
}

/// 검증된 VDI 컨테이너의 read-only 블록 리더.
pub struct VdiReader {
    file: File,
    file_len: u64,
    path: PathBuf,
    header: VdiHeader,
    image: VdiImage,
    block_map: Vec<u32>,
    source_snapshot: SourceSnapshot,
}

impl VdiReader {
    /// VDI를 쓰기 권한 없이 열고 헤더·블록 맵·물리 범위를 검증한다.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VirtualDiskError> {
        Self::open_with_options(path, &VdiSafetyOptions::from_environment())
    }

    /// 안전 검사 설정을 적용해 VDI를 읽기 전용으로 연다.
    pub fn open_with_options(
        path: impl AsRef<Path>,
        options: &VdiSafetyOptions,
    ) -> Result<Self, VirtualDiskError> {
        let path = path.as_ref().to_path_buf();
        reject_lock_artifacts(&path)?;
        probe_running_vm(&path, options.vboxmanage_path.as_deref())?;
        let source_snapshot = source_snapshot(&path)?;
        let mut file = File::open(&path).map_err(|source| VirtualDiskError::Io {
            operation: super::IoOperation::Open,
            source,
        })?;
        let file_len = file
            .metadata()
            .map_err(|source| VirtualDiskError::Io {
                operation: super::IoOperation::Open,
                source,
            })?
            .len();

        if file_len < HEADER_SIZE as u64 {
            return Err(corrupt(format!(
                "헤더가 512바이트보다 짧습니다: {file_len}"
            )));
        }

        let raw = read_exact_at(&mut file, 0, HEADER_SIZE)?;
        let header = parse_header(&raw)?;
        validate_header(&header, file_len)?;

        let map_size = block_map_size(&header)?;
        if map_size > MAX_BLOCK_MAP_BYTES {
            return Err(corrupt(format!(
                "블록 맵이 안전한 메모리 한도를 초과합니다: {map_size}"
            )));
        }
        let map_bytes = read_exact_at(&mut file, header.offset_bmap as u64, map_size as usize)?;
        let block_map = parse_block_map(&map_bytes, &header, file_len)?;

        let image_type = header.image_type;
        let image = VdiImage {
            path: path.clone(),
            version: header.version,
            image_type,
            disk_size_bytes: header.disk_size,
            sector_size: header.sector_size,
        };

        let reader = Self {
            file,
            file_len,
            path: path.clone(),
            header,
            image,
            block_map,
            source_snapshot,
        };
        reader.ensure_source_stable_at(&path)?;
        Ok(reader)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn image(&self) -> &VdiImage {
        &self.image
    }

    pub fn header(&self) -> &VdiHeader {
        &self.header
    }

    pub fn block_map(&self) -> &[u32] {
        &self.block_map
    }

    pub fn file_len(&self) -> u64 {
        self.file_len
    }

    /// 가상 디스크 offset에서 요청한 바이트를 읽는다.
    ///
    /// 미할당·discarded 블록은 파일에 존재하지 않아도 0으로 채운다.
    pub fn read_at(&mut self, offset: u64, buffer: &mut [u8]) -> Result<usize, VirtualDiskError> {
        self.ensure_source_stable()?;
        let length = buffer.len() as u64;
        if offset > self.header.disk_size || length > self.header.disk_size.saturating_sub(offset) {
            return Err(VirtualDiskError::BoundsViolation {
                offset,
                length,
                capacity: self.header.disk_size,
            });
        }
        if buffer.is_empty() {
            return Ok(0);
        }

        let mut done = 0usize;
        while done < buffer.len() {
            let virtual_offset = offset + done as u64;
            let block_index = (virtual_offset / self.header.block_size as u64) as usize;
            let offset_in_block = virtual_offset % self.header.block_size as u64;
            let chunk = (self.header.block_size as u64 - offset_in_block)
                .min((buffer.len() - done) as u64) as usize;
            let map_entry = self.block_map[block_index];

            if map_entry == BLOCK_UNALLOCATED || map_entry == BLOCK_DISCARDED {
                buffer[done..done + chunk].fill(0);
            } else {
                let physical_offset = physical_offset(&self.header, map_entry, offset_in_block)?;
                let bytes = read_exact_at(&mut self.file, physical_offset, chunk)?;
                buffer[done..done + chunk].copy_from_slice(&bytes);
            }
            done += chunk;
        }

        self.ensure_source_stable()?;
        Ok(done)
    }

    fn ensure_source_stable(&self) -> Result<(), VirtualDiskError> {
        let current = source_snapshot(&self.path)?;
        if current != self.source_snapshot {
            return Err(VirtualDiskError::SourceChanged(format!(
                "파일 크기 또는 수정 시각이 열기 이후 변경되었습니다: {}",
                self.path.display()
            )));
        }
        Ok(())
    }
}

impl VdiReader {
    fn ensure_source_stable_at(&self, path: &Path) -> Result<(), VirtualDiskError> {
        let current = source_snapshot(path)?;
        if current != self.source_snapshot {
            return Err(VirtualDiskError::SourceChanged(format!(
                "파일 크기 또는 수정 시각이 열기 중 변경되었습니다: {}",
                path.display()
            )));
        }
        Ok(())
    }
}

fn source_snapshot(path: &Path) -> Result<SourceSnapshot, VirtualDiskError> {
    let metadata = fs::metadata(path).map_err(|source| VirtualDiskError::Io {
        operation: super::IoOperation::Open,
        source,
    })?;
    let modified = metadata.modified().map_err(|source| VirtualDiskError::Io {
        operation: super::IoOperation::Open,
        source,
    })?;
    Ok(SourceSnapshot {
        file_len: metadata.len(),
        modified,
    })
}

fn reject_lock_artifacts(path: &Path) -> Result<(), VirtualDiskError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let entries = fs::read_dir(parent).map_err(|source| VirtualDiskError::Io {
        operation: super::IoOperation::ListDirectory,
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| VirtualDiskError::Io {
            operation: super::IoOperation::ListDirectory,
            source,
        })?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name.ends_with(".lck") {
            return Err(VirtualDiskError::ReadOnlyViolation(format!(
                "VirtualBox 잠금 표식이 있어 열지 않았습니다: {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn probe_running_vm(path: &Path, vboxmanage_path: Option<&Path>) -> Result<(), VirtualDiskError> {
    let Some(vboxmanage_path) = vboxmanage_path else {
        return Ok(());
    };

    let output = Command::new(vboxmanage_path)
        .args(["list", "runningvms"])
        .output()
        .map_err(|source| {
            VirtualDiskError::ReadOnlyViolation(format!(
                "VBoxManage 실행 중 VM 조회에 실패했습니다: {source}"
            ))
        })?;
    if !output.status.success() {
        return Err(VirtualDiskError::ReadOnlyViolation(format!(
            "VBoxManage 실행 중 VM 조회가 실패했습니다: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Some(uuid) = line
            .split('{')
            .nth(1)
            .and_then(|part| part.split('}').next())
        else {
            continue;
        };
        let info = Command::new(vboxmanage_path)
            .args(["showvminfo", uuid, "--machinereadable"])
            .output()
            .map_err(|source| {
                VirtualDiskError::ReadOnlyViolation(format!(
                    "실행 중 VM 디스크 조회에 실패했습니다: {source}"
                ))
            })?;
        if !info.status.success() {
            return Err(VirtualDiskError::ReadOnlyViolation(format!(
                "실행 중 VM 디스크 조회가 실패했습니다: {}",
                String::from_utf8_lossy(&info.stderr).trim()
            )));
        }
        if vm_info_contains_path(&String::from_utf8_lossy(&info.stdout), path) {
            return Err(VirtualDiskError::ReadOnlyViolation(format!(
                "실행 중인 VM이 VDI를 사용 중이어서 열지 않았습니다: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn vm_info_contains_path(info: &str, target: &Path) -> bool {
    let target = normalized_path(target);
    info.lines()
        .filter_map(|line| line.split_once('='))
        .any(|(_, value)| {
            let value = value.trim().trim_matches('"');
            value.to_ascii_lowercase().contains(".vdi")
                && normalized_path(Path::new(value)) == target
        })
}

fn normalized_path(path: &Path) -> String {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    path.to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase()
}

fn parse_header(raw: &[u8]) -> Result<VdiHeader, VirtualDiskError> {
    let signature = le_u32(raw, 0x40)?;
    if signature != SIGNATURE {
        return Err(corrupt(format!(
            "VDI 시그니처가 다릅니다: 0x{signature:08x}"
        )));
    }

    let version = VdiVersion::from_raw(le_u32(raw, 0x44)?);
    if version != VdiVersion::CURRENT {
        return Err(VirtualDiskError::UnsupportedFormat {
            kind: UnsupportedFormatKind::VdiVersion,
            detail: format!("지원 버전은 1.1이며 현재 값은 {version}입니다"),
        });
    }

    let raw_image_type = le_u32(raw, 0x4c)?;
    let image_type = match raw_image_type {
        IMAGE_TYPE_DYNAMIC => VdiImageType::Dynamic,
        IMAGE_TYPE_FIXED => VdiImageType::Fixed,
        IMAGE_TYPE_DIFFERENCING => VdiImageType::Differencing,
        other => {
            return Err(VirtualDiskError::UnsupportedFormat {
                kind: UnsupportedFormatKind::VdiImageType,
                detail: format!("지원하지 않는 VDI 이미지 유형: {other}"),
            })
        }
    };

    Ok(VdiHeader {
        version,
        header_size: le_u32(raw, 0x48)?,
        image_type,
        image_flags: le_u32(raw, 0x50)?,
        offset_bmap: le_u32(raw, 0x154)?,
        offset_data: le_u32(raw, 0x158)?,
        sector_size: le_u32(raw, 0x168)?,
        // VDI 1.1의 디스크 크기·블록 필드는 0x170부터 시작한다. 0x16c는
        // 예약 필드이고, VirtualBox가 실제로 생성한 이미지도 이 표준 배치를 쓴다.
        disk_size: le_u64(raw, 0x170)?,
        block_size: le_u32(raw, 0x178)?,
        block_extra: le_u32(raw, 0x17c)?,
        blocks_in_image: le_u32(raw, 0x180)?,
        blocks_allocated: le_u32(raw, 0x184)?,
    })
}

fn validate_header(header: &VdiHeader, file_len: u64) -> Result<(), VirtualDiskError> {
    if header.image_type == VdiImageType::Differencing {
        return Err(VirtualDiskError::UnsupportedFormat {
            kind: UnsupportedFormatKind::VdiImageType,
            detail: "차등 이미지와 부모 체인은 VDE-004에서 지원하지 않습니다".to_string(),
        });
    }
    if header.header_size < MIN_HEADER_MAIN_SIZE {
        return Err(corrupt(format!(
            "VDI 주 헤더 크기가 너무 작습니다: {}",
            header.header_size
        )));
    }
    if header.sector_size != SECTOR_SIZE {
        return Err(VirtualDiskError::UnsupportedFormat {
            kind: UnsupportedFormatKind::SectorSize,
            detail: format!(
                "VDI 섹터 크기는 512바이트여야 합니다: {}",
                header.sector_size
            ),
        });
    }
    if header.block_size == 0 || !header.block_size.is_multiple_of(header.sector_size) {
        return Err(corrupt(format!(
            "VDI 블록 크기가 섹터 경계에 맞지 않습니다: {}",
            header.block_size
        )));
    }
    if !(header.offset_bmap as u64).is_multiple_of(header.sector_size as u64)
        || !(header.offset_data as u64).is_multiple_of(header.sector_size as u64)
    {
        return Err(corrupt(
            "블록 맵 또는 데이터 오프셋이 섹터 경계에 맞지 않습니다",
        ));
    }

    let capacity = (header.blocks_in_image as u64)
        .checked_mul(header.block_size as u64)
        .ok_or_else(|| corrupt("가상 디스크 용량 계산이 오버플로됩니다"))?;
    if header.disk_size > capacity {
        return Err(corrupt(format!(
            "디스크 크기가 블록 맵 용량을 초과합니다: disk={}, capacity={capacity}",
            header.disk_size
        )));
    }
    let map_size = block_map_size(header)?;
    let map_end = (header.offset_bmap as u64)
        .checked_add(map_size)
        .ok_or_else(|| corrupt("블록 맵 끝 오프셋이 오버플로됩니다"))?;
    if (header.offset_bmap as u64) < HEADER_SIZE as u64 || map_end > file_len {
        return Err(corrupt(format!(
            "블록 맵 범위가 파일을 벗어납니다: offset={}, size={map_size}, file={file_len}",
            header.offset_bmap
        )));
    }
    if (header.offset_data as u64) < map_end || (header.offset_data as u64) > file_len {
        return Err(corrupt(format!(
            "데이터 오프셋이 블록 맵 뒤에 있지 않습니다: data={}, map_end={map_end}, file={file_len}",
            header.offset_data
        )));
    }
    Ok(())
}

fn block_map_size(header: &VdiHeader) -> Result<u64, VirtualDiskError> {
    let raw_size = (header.blocks_in_image as u64)
        .checked_mul(4)
        .ok_or_else(|| corrupt("블록 맵 크기 계산이 오버플로됩니다"))?;
    let sector = header.sector_size as u64;
    if sector == 0 {
        return Err(corrupt("섹터 크기가 0입니다"));
    }
    let remainder = raw_size % sector;
    if remainder == 0 {
        Ok(raw_size)
    } else {
        raw_size
            .checked_add(sector - remainder)
            .ok_or_else(|| corrupt("정렬된 블록 맵 크기가 오버플로됩니다"))
    }
}

fn parse_block_map(
    raw: &[u8],
    header: &VdiHeader,
    file_len: u64,
) -> Result<Vec<u32>, VirtualDiskError> {
    let count = usize::try_from(header.blocks_in_image)
        .map_err(|_| corrupt("블록 맵 항목 수를 메모리 크기로 변환할 수 없습니다"))?;
    let mut result = Vec::with_capacity(count);
    let mut seen = HashSet::with_capacity(count);
    let mut allocated = 0u32;

    for index in 0..count {
        let offset = index
            .checked_mul(4)
            .ok_or_else(|| corrupt("블록 맵 항목 오프셋이 오버플로됩니다"))?;
        let entry = le_u32(raw, offset)?;
        if entry < BLOCK_DISCARDED {
            if entry >= header.blocks_in_image {
                return Err(corrupt(format!(
                    "블록 맵 항목 {index}가 이미지 블록 수를 벗어납니다: {entry}"
                )));
            }
            if !seen.insert(entry) {
                return Err(corrupt(format!("물리 블록이 중복 매핑되었습니다: {entry}")));
            }
            allocated = allocated
                .checked_add(1)
                .ok_or_else(|| corrupt("할당 블록 수가 오버플로됩니다"))?;
            let end = physical_offset(header, entry, header.block_size as u64)?;
            if end > file_len {
                return Err(corrupt(format!(
                    "할당된 블록 {entry}의 끝이 파일을 벗어납니다: {end} > {file_len}"
                )));
            }
        } else if entry != BLOCK_UNALLOCATED && entry != BLOCK_DISCARDED {
            return Err(corrupt(format!("알 수 없는 블록 맵 표식: 0x{entry:08x}")));
        }
        result.push(entry);
    }

    if allocated != header.blocks_allocated {
        return Err(corrupt(format!(
            "헤더의 할당 블록 수와 맵이 다릅니다: header={}, map={allocated}",
            header.blocks_allocated
        )));
    }
    Ok(result)
}

fn physical_offset(
    header: &VdiHeader,
    map_entry: u32,
    offset_in_block: u64,
) -> Result<u64, VirtualDiskError> {
    let stride = (header.block_size as u64)
        .checked_add(header.block_extra as u64)
        .ok_or_else(|| corrupt("물리 블록 stride가 오버플로됩니다"))?;
    let block = (map_entry as u64)
        .checked_mul(stride)
        .ok_or_else(|| corrupt("물리 블록 오프셋이 오버플로됩니다"))?;
    (header.offset_data as u64)
        .checked_add(block)
        .and_then(|offset| offset.checked_add(header.block_extra as u64))
        .and_then(|offset| offset.checked_add(offset_in_block))
        .ok_or_else(|| corrupt("물리 블록 오프셋이 오버플로됩니다"))
}

fn read_exact_at(file: &mut File, offset: u64, length: usize) -> Result<Vec<u8>, VirtualDiskError> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|source| VirtualDiskError::Io {
            operation: super::IoOperation::Seek,
            source,
        })?;
    let mut buffer = vec![0u8; length];
    file.read_exact(&mut buffer)
        .map_err(|source| VirtualDiskError::Io {
            operation: super::IoOperation::Read,
            source,
        })?;
    Ok(buffer)
}

fn le_u32(raw: &[u8], offset: usize) -> Result<u32, VirtualDiskError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or_else(|| corrupt("VDI 헤더 필드가 잘렸습니다"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4-byte slice")))
}

fn le_u64(raw: &[u8], offset: usize) -> Result<u64, VirtualDiskError> {
    let bytes = raw
        .get(offset..offset + 8)
        .ok_or_else(|| corrupt("VDI 헤더 필드가 잘렸습니다"))?;
    Ok(u64::from_le_bytes(bytes.try_into().expect("8-byte slice")))
}

fn corrupt(detail: impl Into<String>) -> VirtualDiskError {
    VirtualDiskError::CorruptImage(detail.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_disk::{
        ntfs::NtfsGuestFileSource, partition::discover_partitions, GuestFileSource, GuestFileSystem,
        PartitionTableKind,
    };
    use std::{
        fs,
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
    };

    const BLOCK_SIZE: u32 = 512;

    #[test]
    fn reads_dynamic_image_and_returns_zero_for_unallocated_blocks() {
        let path = write_fixture(IMAGE_TYPE_DYNAMIC, &[1, BLOCK_UNALLOCATED, 0], 2);
        let original = fs::read(&path).unwrap();
        let mut reader = VdiReader::open(&path).unwrap();
        let mut contents = vec![0u8; 3 * BLOCK_SIZE as usize];

        assert_eq!(reader.read_at(0, &mut contents).unwrap(), contents.len());
        assert_eq!(&contents[..512], vec![b'B'; 512].as_slice());
        assert_eq!(&contents[512..1024], vec![0; 512].as_slice());
        assert_eq!(&contents[1024..], vec![b'A'; 512].as_slice());
        assert_eq!(reader.image().image_type, VdiImageType::Dynamic);
        assert_eq!(fs::read(&path).unwrap(), original);
        remove_fixture(&path);
    }

    #[test]
    fn reads_fixed_image_using_the_block_map() {
        let path = write_fixture(IMAGE_TYPE_FIXED, &[0, 1], 2);
        let mut reader = VdiReader::open(&path).unwrap();
        let mut contents = vec![0u8; 768];

        reader.read_at(256, &mut contents).unwrap();

        assert_eq!(&contents[..256], vec![b'A'; 256].as_slice());
        assert_eq!(&contents[256..], vec![b'B'; 512].as_slice());
        assert_eq!(reader.image().image_type, VdiImageType::Fixed);
        remove_fixture(&path);
    }

    #[test]
    fn rejects_out_of_range_map_entries_and_metadata_overflow() {
        let invalid_entry = write_fixture(IMAGE_TYPE_DYNAMIC, &[3, BLOCK_UNALLOCATED, 0], 2);
        assert!(matches!(
            VdiReader::open(&invalid_entry),
            Err(VirtualDiskError::CorruptImage(_))
        ));
        remove_fixture(&invalid_entry);

        let overflow = fixture_path();
        let mut header = base_header(IMAGE_TYPE_DYNAMIC, u32::MAX, 0);
        write_u32(&mut header, 0x154, 512);
        write_u32(&mut header, 0x158, 512);
        let mut file = fs::File::create(&overflow).unwrap();
        file.write_all(&header).unwrap();
        assert!(matches!(
            VdiReader::open(&overflow),
            Err(VirtualDiskError::CorruptImage(_))
        ));
        remove_fixture(&overflow);
    }

    #[test]
    fn rejects_differencing_images_before_reading_the_block_map() {
        let path = write_fixture(IMAGE_TYPE_DIFFERENCING, &[0, 1], 2);

        assert!(matches!(
            VdiReader::open(&path),
            Err(VirtualDiskError::UnsupportedFormat {
                kind: UnsupportedFormatKind::VdiImageType,
                ..
            })
        ));
        remove_fixture(&path);
    }

    #[test]
    fn rejects_virtualbox_lock_artifacts_before_opening() {
        let path = write_fixture(IMAGE_TYPE_DYNAMIC, &[0], 1);
        let lock_path = path.with_file_name(format!(
            "{}.lck",
            path.file_name().unwrap().to_string_lossy()
        ));
        fs::create_dir(&lock_path).unwrap();

        assert!(matches!(
            VdiReader::open(&path),
            Err(VirtualDiskError::ReadOnlyViolation(message)) if message.contains("잠금")
        ));

        let _ = fs::remove_dir(&lock_path);
        remove_fixture(&path);
    }

    #[test]
    fn rejects_source_changes_after_open() {
        let path = write_fixture(IMAGE_TYPE_DYNAMIC, &[0], 1);
        let mut reader = VdiReader::open(&path).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(2048)
            .unwrap();

        let mut buffer = [0u8; 1];
        assert!(matches!(
            reader.read_at(0, &mut buffer),
            Err(VirtualDiskError::SourceChanged(_))
        ));
        remove_fixture(&path);
    }

    #[test]
    fn rejects_source_mtime_changes_even_when_size_is_unchanged() {
        let path = write_fixture(IMAGE_TYPE_DYNAMIC, &[0], 1);
        let mut reader = VdiReader::open(&path).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .write_all(b"Z")
            .unwrap();

        let mut buffer = [0u8; 1];
        assert!(matches!(
            reader.read_at(0, &mut buffer),
            Err(VirtualDiskError::SourceChanged(_))
        ));
        remove_fixture(&path);
    }

    #[test]
    fn opens_a_read_only_handle() {
        let path = write_fixture(IMAGE_TYPE_DYNAMIC, &[0], 1);
        let reader = VdiReader::open(&path).unwrap();
        let mut clone = reader.file.try_clone().unwrap();

        assert!(clone.write_all(&[0]).is_err());
        remove_fixture(&path);
    }

    #[test]
    fn detects_a_running_vm_disk_from_machine_readable_info() {
        let path = write_fixture(IMAGE_TYPE_DYNAMIC, &[0], 1);
        let info = format!("SATA-0-0=\"{}\"\n", path.display());

        assert!(vm_info_contains_path(&info, &path));
        remove_fixture(&path);
    }

    #[test]
    fn reads_bundled_ntfs_through_a_synthetic_vdi_without_mutating_the_source() {
        let path = write_ntfs_vdi_fixture();
        let original = fs::read(&path).unwrap();
        let mut reader = VdiReader::open(&path).unwrap();

        let partitions = discover_partitions(&mut reader).unwrap();
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].filesystem, Some(GuestFileSystem::ntfs_3_1()));

        let mut source = NtfsGuestFileSource::open(reader, partitions[0].clone()).unwrap();
        let root = source.root().unwrap();
        let entries = source.list_directory(&root).unwrap();
        assert!(!entries.is_empty());

        let file = entries
            .iter()
            .find(|entry| !entry.is_directory() && entry.size_bytes > 0)
            .expect("bundled NTFS fixture should contain a readable file");
        let read_length = usize::try_from(file.size_bytes.min(64)).unwrap();
        let mut buffer = vec![0; read_length];
        assert_eq!(source.read_at(file, 0, &mut buffer).unwrap(), read_length);
        assert_eq!(fs::read(&path).unwrap(), original);

        remove_fixture(&path);
    }

    #[test]
    fn reads_unsupported_partition_fixture_without_claiming_ntfs() {
        let path = write_unsupported_partition_vdi_fixture();
        let mut reader = VdiReader::open(&path).unwrap();

        let partitions = discover_partitions(&mut reader).unwrap();
        assert_eq!(partitions.len(), 1);
        assert_eq!(partitions[0].filesystem, None);
        assert_eq!(partitions[0].table, PartitionTableKind::Mbr);
        assert_eq!(partitions[0].start_lba, 1);
        assert_eq!(partitions[0].sector_count, 1);

        remove_fixture(&path);
    }

    #[test]
    fn exports_ntfs_vdi_fixture_when_requested() {
        let Ok(destination) = std::env::var("GPUI_CONVENIENCE_TOOLS_EXPORT_VDI_FIXTURE") else {
            return;
        };
        let destination = PathBuf::from(destination);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let source = write_ntfs_vdi_fixture();
        fs::copy(&source, &destination).unwrap();
        remove_fixture(&source);
        assert!(destination.is_file());
    }

    #[test]
    fn exports_unsupported_partition_vdi_fixture_when_requested() {
        let Ok(destination) =
            std::env::var("GPUI_CONVENIENCE_TOOLS_EXPORT_UNSUPPORTED_VDI_FIXTURE")
        else {
            return;
        };
        let destination = PathBuf::from(destination);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let source = write_unsupported_partition_vdi_fixture();
        fs::copy(&source, &destination).unwrap();
        remove_fixture(&source);
        assert!(destination.is_file());
    }

    #[test]
    fn exports_ntfs_raw_fixture_when_requested() {
        let Ok(destination) = std::env::var("GPUI_CONVENIENCE_TOOLS_EXPORT_NTFS_RAW_FIXTURE")
        else {
            return;
        };
        let destination = PathBuf::from(destination);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let source = write_ntfs_raw_fixture();
        fs::copy(&source, &destination).unwrap();
        remove_fixture(&source);
        assert!(destination.is_file());
    }

    fn write_fixture(image_type: u32, map: &[u32], allocated: u32) -> PathBuf {
        let path = fixture_path();
        let mut header = base_header(image_type, map.len() as u32, allocated);
        write_u32(&mut header, 0x154, 512);
        write_u32(&mut header, 0x158, 1024);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(&header).unwrap();
        let mut map_bytes = vec![0u8; 512];
        for (index, entry) in map.iter().enumerate() {
            write_u32(&mut map_bytes, index * 4, *entry);
        }
        file.write_all(&map_bytes).unwrap();
        for block in [vec![b'A'; 512], vec![b'B'; 512]] {
            file.write_all(&block).unwrap();
        }
        path
    }

    fn write_ntfs_vdi_fixture() -> PathBuf {
        let disk = ntfs_disk_bytes();
        let block_count = u32::try_from(disk.len() / BLOCK_SIZE as usize).unwrap();
        let map_size = block_map_size(&VdiHeader {
            version: VdiVersion::CURRENT,
            header_size: MIN_HEADER_MAIN_SIZE,
            image_type: VdiImageType::Dynamic,
            image_flags: 0,
            offset_bmap: 512,
            offset_data: 0,
            sector_size: SECTOR_SIZE,
            disk_size: disk.len() as u64,
            block_size: BLOCK_SIZE,
            block_extra: 0,
            blocks_in_image: block_count,
            blocks_allocated: block_count,
        })
        .unwrap();
        let data_offset = 512 + map_size;

        let path = fixture_path();
        let mut header = base_header(IMAGE_TYPE_DYNAMIC, block_count, block_count);
        write_u32(&mut header, 0x154, 512);
        write_u32(&mut header, 0x158, u32::try_from(data_offset).unwrap());
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(&header).unwrap();

        for entry in 0..block_count {
            file.write_all(&entry.to_le_bytes()).unwrap();
        }
        file.write_all(&vec![
            0;
            usize::try_from(map_size).unwrap()
                - block_count as usize * 4
        ])
        .unwrap();
        file.write_all(&disk).unwrap();
        path
    }

    fn write_ntfs_raw_fixture() -> PathBuf {
        let path = fixture_path().with_file_name("disk.raw");
        fs::write(&path, ntfs_disk_bytes()).unwrap();
        path
    }

    fn write_unsupported_partition_vdi_fixture() -> PathBuf {
        let path = fixture_path();
        let mut header = base_header(IMAGE_TYPE_DYNAMIC, 2, 2);
        write_u32(&mut header, 0x154, 512);
        write_u32(&mut header, 0x158, 1024);

        let mut mbr = vec![0u8; BLOCK_SIZE as usize];
        mbr[446 + 4] = 0x83;
        mbr[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
        mbr[446 + 12..446 + 16].copy_from_slice(&1u32.to_le_bytes());
        mbr[510..512].copy_from_slice(&[0x55, 0xaa]);

        let mut file = fs::File::create(&path).unwrap();
        file.write_all(&header).unwrap();
        let mut block_map = vec![0u8; 512];
        block_map[..8].copy_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0]);
        file.write_all(&block_map).unwrap();
        file.write_all(&mbr).unwrap();
        file.write_all(&vec![0; BLOCK_SIZE as usize]).unwrap();
        path
    }

    fn ntfs_disk_bytes() -> Vec<u8> {
        let ntfs_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("ntfs-testfs1.img");
        let ntfs = fs::read(ntfs_path).expect("bundled NTFS fixture must be present");
        assert!(ntfs.len().is_multiple_of(BLOCK_SIZE as usize));

        let mut disk = vec![0u8; BLOCK_SIZE as usize];
        disk[510..512].copy_from_slice(&[0x55, 0xaa]);
        disk[446 + 4] = 0x07;
        disk[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
        disk[446 + 12..446 + 16].copy_from_slice(
            &u32::try_from(ntfs.len() / BLOCK_SIZE as usize)
                .unwrap()
                .to_le_bytes(),
        );
        disk.extend_from_slice(&ntfs);
        disk
    }

    fn base_header(image_type: u32, blocks: u32, allocated: u32) -> Vec<u8> {
        let mut header = vec![0u8; HEADER_SIZE];
        write_u32(&mut header, 0x40, SIGNATURE);
        write_u32(&mut header, 0x44, VdiVersion::CURRENT.raw());
        write_u32(&mut header, 0x48, MIN_HEADER_MAIN_SIZE);
        write_u32(&mut header, 0x4c, image_type);
        write_u32(&mut header, 0x168, SECTOR_SIZE);
        write_u64(&mut header, 0x170, blocks as u64 * BLOCK_SIZE as u64);
        write_u32(&mut header, 0x178, BLOCK_SIZE);
        write_u32(&mut header, 0x17c, 0);
        write_u32(&mut header, 0x180, blocks);
        write_u32(&mut header, 0x184, allocated);
        header
    }

    fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
        buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
        buffer[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn fixture_path() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "gpui-convenience-vde005-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory.join("disk.vdi")
    }

    fn remove_fixture(path: &Path) {
        let _ = fs::remove_file(path);
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}
