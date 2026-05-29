/// 빌드 스크립트
///
/// 릴리즈 빌드에서 Windows 링커의 기본 매니페스트 자동 생성을 억제한다.
/// gpui의 windows-manifest 피처가 이미 DPI/Common Controls 매니페스트를
/// 임베드하므로, 링커 자동 생성을 허용하면 RT_MANIFEST(ID=1) 중복이 발생한다.
fn main() {
    #[cfg(target_os = "windows")]
    {
        // 링커 자동 매니페스트 생성 비활성화 (gpui 임베드 매니페스트와 중복 방지)
        println!("cargo:rustc-link-arg=/MANIFEST:NO");
    }
}
