#!/usr/bin/env bash
# 在 Linux 上构建并打包 MetaAlign：产出 dist/*.tar.gz 与 dist/*.deb。
# 用法：bash scripts/package-linux.sh [--skip-build]

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
root="$(pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
arch="x86_64"
name="MetaAlign-${version}-linux-${arch}"
dist="$root/dist"
stage="$dist/stage/$name"

if [ "${1:-}" != "--skip-build" ]; then
    echo "==> 构建 release"
    cargo build --release -p metaalign-gui
fi

bin="$root/target/release/MetaAlign"
[ -x "$bin" ] || { echo "缺少 $bin，先构建" >&2; exit 1; }

echo "==> 组装 $name"
rm -rf "$dist/stage"
mkdir -p "$stage"
install -m 755 "$bin" "$stage/MetaAlign"
install -m 644 "$root/crates/metaalign-gui/assets/icon.png" "$stage/metaalign.png"
install -m 644 "$root/packaging/linux/metaalign.desktop" "$stage/metaalign.desktop"
install -m 755 "$root/packaging/linux/install.sh" "$stage/install.sh"
install -m 644 "$root/README.md" "$stage/README.md"

echo "==> 打 tar.gz"
tar -C "$dist/stage" --owner=0 --group=0 -czf "$dist/$name.tar.gz" "$name"

echo "==> 打 deb"
debroot="$dist/deb"
rm -rf "$debroot"
mkdir -p "$debroot/DEBIAN" \
         "$debroot/usr/bin" \
         "$debroot/usr/share/applications" \
         "$debroot/usr/share/icons/hicolor/256x256/apps" \
         "$debroot/usr/share/doc/metaalign"
install -m 755 "$bin" "$debroot/usr/bin/MetaAlign"
install -m 644 "$root/crates/metaalign-gui/assets/icon.png" \
    "$debroot/usr/share/icons/hicolor/256x256/apps/metaalign.png"
sed 's|__PREFIX__|/usr|g' "$root/packaging/linux/metaalign.desktop" \
    > "$debroot/usr/share/applications/metaalign.desktop"
chmod 644 "$debroot/usr/share/applications/metaalign.desktop"
install -m 644 "$root/README.md" "$debroot/usr/share/doc/metaalign/README.md"

installed_kb="$(du -sk --exclude=DEBIAN "$debroot" | cut -f1)"
cat > "$debroot/DEBIAN/control" <<EOF
Package: metaalign
Version: ${version}
Architecture: amd64
Maintainer: Demon1110 <Demon1110@users.noreply.github.com>
Installed-Size: ${installed_kb}
Section: utils
Priority: optional
Depends: libgl1, libxkbcommon0, libx11-6, libfontconfig1
Recommends: fonts-noto-cjk | fonts-wqy-microhei
Description: 批量整理文件时间戳与 Office 文档属性
 把一堆文件的创建/修改/访问时间、只读/隐藏/系统/存档位按同一套规则改写，
 并批量覆盖 Office 文档的作者、标题、公司、修订号等内置属性，
 支持在给定时间区间内随机填充时间与随机生成作者姓名。
EOF

dpkg-deb --root-owner-group -Zxz --build "$debroot" \
    "$dist/MetaAlign_${version}_amd64.deb" > /dev/null

echo "==> 产物"
ls -lh "$dist/$name.tar.gz" "$dist/MetaAlign_${version}_amd64.deb"
cd "$dist"
sha256sum "$name.tar.gz" > "$name.tar.gz.sha256"
sha256sum "MetaAlign_${version}_amd64.deb" > "MetaAlign_${version}_amd64.deb.sha256"
cat ./*.sha256
