#!/usr/bin/env bash
#
# macOS 앱 번들(.app)과 배포용 디스크 이미지(.dmg)를 만든다.
#
# Windows의 `installer/windows/build-installer.ps1`에 대응하는 스크립트다.
# macOS에서만 동작하며 GitHub Actions의 macos 러너에서 호출된다.
#
# 사용법:
#   bash installer/macos/build-app.sh [버전]
#
# 버전을 생략하면 app/Cargo.toml의 값을 쓴다. 태그 빌드에서는 `v1.2.3` 같은 태그명을
# 그대로 넘겨도 되며 앞의 `v`는 제거된다.
#
# 결과물:
#   target/macos/gpui-convenience-tools.app
#   target/macos/gpui-convenience-tools-<버전>-universal.dmg
#
# 주의: 코드 서명·공증(notarization)은 하지 않는다. 서명되지 않은 앱이므로 사용자는
# 첫 실행 시 Gatekeeper를 우회해야 한다(릴리즈 노트와 README에 안내).

set -euo pipefail

PACKAGE_NAME="gpui-convenience-tools"
APP_NAME="gpui-convenience-tools"
# 배포 주체가 바뀌면 이 값을 함께 바꾼다. 번들 ID는 macOS가 앱을 식별하는 키다.
BUNDLE_ID="${BUNDLE_ID:-com.github.ultimalife.gpui-convenience-tools}"
MIN_MACOS_VERSION="11.0"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "이 스크립트는 macOS에서만 실행할 수 있다 (현재: $(uname -s))." >&2
    exit 1
fi

# ── 버전 결정 ────────────────────────────────────────────────────────────────
version="${1:-}"
if [[ -z "$version" ]]; then
    version="$(sed -n 's/^version *= *"\(.*\)"/\1/p' app/Cargo.toml | head -n 1)"
fi
version="${version#v}"
if [[ -z "$version" ]]; then
    echo "버전을 확인할 수 없다." >&2
    exit 1
fi
# CFBundleVersion은 숫자와 점만 허용한다. `1.2.3-rc1` 같은 태그는 접미사를 떼어 쓴다.
numeric_version="$(printf '%s' "$version" | sed 's/[^0-9.].*$//')"
numeric_version="${numeric_version:-0.0.0}"

echo "==> 버전: $version (CFBundleVersion: $numeric_version)"

# ── 유니버설 바이너리 빌드 ───────────────────────────────────────────────────
# Apple Silicon과 Intel Mac을 한 파일로 지원한다. 러너 아키텍처에 관계없이
# 두 타깃을 모두 만든 뒤 lipo로 합친다.
for target in aarch64-apple-darwin x86_64-apple-darwin; do
    echo "==> cargo build --release --target $target"
    rustup target add "$target" >/dev/null
    cargo build -p "$PACKAGE_NAME" --release --target "$target"
done

staging="target/macos"
app_dir="$staging/$APP_NAME.app"
macos_dir="$app_dir/Contents/MacOS"
resources_dir="$app_dir/Contents/Resources"

rm -rf "$app_dir"
mkdir -p "$macos_dir" "$resources_dir"

echo "==> lipo로 유니버설 바이너리 생성"
lipo -create \
    "target/aarch64-apple-darwin/release/$PACKAGE_NAME" \
    "target/x86_64-apple-darwin/release/$PACKAGE_NAME" \
    -output "$macos_dir/$APP_NAME"
chmod +x "$macos_dir/$APP_NAME"
lipo -info "$macos_dir/$APP_NAME"

# ── Info.plist ───────────────────────────────────────────────────────────────
cat > "$app_dir/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>GPUI 편의 도구</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$numeric_version</string>
    <key>LSMinimumSystemVersion</key>
    <string>$MIN_MACOS_VERSION</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.utilities</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
PLIST

# 서명하지 않더라도 ad-hoc 서명을 붙여 두면 Apple Silicon에서 "손상된 앱" 대신
# 정상적인 미확인 개발자 경고가 뜬다. arm64 바이너리는 서명이 아예 없으면 실행되지 않는다.
echo "==> ad-hoc 코드 서명"
codesign --force --deep --sign - "$app_dir"
codesign --verify --verbose "$app_dir"

# ── DMG ──────────────────────────────────────────────────────────────────────
dmg_path="$staging/$APP_NAME-$version-universal.dmg"
dmg_root="$staging/dmg"
rm -rf "$dmg_root" "$dmg_path"
mkdir -p "$dmg_root"
cp -R "$app_dir" "$dmg_root/"
# 드래그 앤 드롭 설치를 위해 응용 프로그램 폴더 별칭을 넣는다.
ln -s /Applications "$dmg_root/Applications"

echo "==> DMG 생성"
hdiutil create \
    -volname "$APP_NAME" \
    -srcfolder "$dmg_root" \
    -ov -format UDZO \
    "$dmg_path"

rm -rf "$dmg_root"

echo ""
echo "완료:"
echo "  $app_dir"
echo "  $dmg_path"
