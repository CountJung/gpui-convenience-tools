use serde::{Deserialize, Serialize};

/// VDI 탐색기의 기본 레이아웃 폭.
pub const DEFAULT_VIRTUAL_DISK_TREE_WIDTH: f32 = 180.0;
pub const DEFAULT_VIRTUAL_DISK_KIND_WIDTH: f32 = 64.0;
pub const DEFAULT_VIRTUAL_DISK_ATTRIBUTES_WIDTH: f32 = 116.0;
pub const DEFAULT_VIRTUAL_DISK_SIZE_WIDTH: f32 = 96.0;

const MIN_VIRTUAL_DISK_TREE_WIDTH: f32 = 150.0;
const MAX_VIRTUAL_DISK_TREE_WIDTH: f32 = 240.0;
const MIN_VIRTUAL_DISK_KIND_WIDTH: f32 = 60.0;
const MAX_VIRTUAL_DISK_KIND_WIDTH: f32 = 104.0;
const MIN_VIRTUAL_DISK_ATTRIBUTES_WIDTH: f32 = 96.0;
const MAX_VIRTUAL_DISK_ATTRIBUTES_WIDTH: f32 = 176.0;
const MIN_VIRTUAL_DISK_SIZE_WIDTH: f32 = 84.0;
const MAX_VIRTUAL_DISK_SIZE_WIDTH: f32 = 144.0;

/// VDI 탐색기에서 사용자가 조정하는 트리·목록 열 폭.
///
/// 파일명 열은 남은 폭을 사용하므로 별도 고정 폭을 저장하지 않는다. 새 필드는
/// `serde(default)`로 구버전 settings.json을 그대로 읽을 수 있게 한다.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VirtualDiskLayoutConfig {
    #[serde(default = "default_virtual_disk_tree_width")]
    pub tree_width: f32,
    #[serde(default = "default_virtual_disk_kind_width")]
    pub kind_width: f32,
    #[serde(default = "default_virtual_disk_attributes_width")]
    pub attributes_width: f32,
    #[serde(default = "default_virtual_disk_size_width")]
    pub size_width: f32,
}

impl Default for VirtualDiskLayoutConfig {
    fn default() -> Self {
        Self {
            tree_width: DEFAULT_VIRTUAL_DISK_TREE_WIDTH,
            kind_width: DEFAULT_VIRTUAL_DISK_KIND_WIDTH,
            attributes_width: DEFAULT_VIRTUAL_DISK_ATTRIBUTES_WIDTH,
            size_width: DEFAULT_VIRTUAL_DISK_SIZE_WIDTH,
        }
    }
}

fn default_virtual_disk_tree_width() -> f32 {
    DEFAULT_VIRTUAL_DISK_TREE_WIDTH
}

fn default_virtual_disk_kind_width() -> f32 {
    DEFAULT_VIRTUAL_DISK_KIND_WIDTH
}

fn default_virtual_disk_attributes_width() -> f32 {
    DEFAULT_VIRTUAL_DISK_ATTRIBUTES_WIDTH
}

fn default_virtual_disk_size_width() -> f32 {
    DEFAULT_VIRTUAL_DISK_SIZE_WIDTH
}

pub fn normalize_virtual_disk_tree_width(width: f32) -> f32 {
    normalize_virtual_disk_width(
        width,
        DEFAULT_VIRTUAL_DISK_TREE_WIDTH,
        MIN_VIRTUAL_DISK_TREE_WIDTH,
        MAX_VIRTUAL_DISK_TREE_WIDTH,
    )
}

pub fn normalize_virtual_disk_kind_width(width: f32) -> f32 {
    normalize_virtual_disk_width(
        width,
        DEFAULT_VIRTUAL_DISK_KIND_WIDTH,
        MIN_VIRTUAL_DISK_KIND_WIDTH,
        MAX_VIRTUAL_DISK_KIND_WIDTH,
    )
}

pub fn normalize_virtual_disk_attributes_width(width: f32) -> f32 {
    normalize_virtual_disk_width(
        width,
        DEFAULT_VIRTUAL_DISK_ATTRIBUTES_WIDTH,
        MIN_VIRTUAL_DISK_ATTRIBUTES_WIDTH,
        MAX_VIRTUAL_DISK_ATTRIBUTES_WIDTH,
    )
}

pub fn normalize_virtual_disk_size_width(width: f32) -> f32 {
    normalize_virtual_disk_width(
        width,
        DEFAULT_VIRTUAL_DISK_SIZE_WIDTH,
        MIN_VIRTUAL_DISK_SIZE_WIDTH,
        MAX_VIRTUAL_DISK_SIZE_WIDTH,
    )
}

fn normalize_virtual_disk_width(width: f32, default: f32, min: f32, max: f32) -> f32 {
    if width.is_finite() {
        width.clamp(min, max)
    } else {
        default
    }
}
