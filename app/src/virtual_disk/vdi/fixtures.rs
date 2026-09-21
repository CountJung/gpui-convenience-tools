use super::*;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub(super) const BLOCK_SIZE: u32 = 512;

pub(super) fn write_fixture(image_type: u32, map: &[u32], allocated: u32) -> PathBuf {
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

pub(super) fn write_ntfs_vdi_fixture() -> PathBuf {
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

pub(super) fn write_ntfs_raw_fixture() -> PathBuf {
    let path = fixture_path().with_file_name("disk.raw");
    fs::write(&path, ntfs_disk_bytes()).unwrap();
    path
}

pub(super) fn write_unsupported_partition_vdi_fixture() -> PathBuf {
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

pub(super) fn base_header(image_type: u32, blocks: u32, allocated: u32) -> Vec<u8> {
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

pub(super) fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(buffer: &mut [u8], offset: usize, value: u64) {
    buffer[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn fixture_path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "gpui-convenience-vde005-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).unwrap();
    directory.join("disk.vdi")
}

pub(super) fn remove_fixture(path: &std::path::Path) {
    let _ = fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}
