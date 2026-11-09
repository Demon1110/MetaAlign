#!/usr/bin/env bash
# MetaAlign Linux 安装脚本
# 把可执行文件、图标与桌面入口装到 $PREFIX（默认 ~/.local）。
# 系统级安装：PREFIX=/usr/local sudo bash install.sh

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"
appdir="$prefix/share/applications"
icondir="$prefix/share/icons/hicolor/256x256/apps"

install -d "$bindir" "$appdir" "$icondir"
install -m 755 "$here/MetaAlign" "$bindir/MetaAlign"
install -m 644 "$here/metaalign.png" "$icondir/metaalign.png"
sed "s|__PREFIX__|$prefix|g" "$here/metaalign.desktop" > "$appdir/metaalign.desktop"
chmod 644 "$appdir/metaalign.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$appdir" >/dev/null 2>&1 || true
fi

echo "已安装：$bindir/MetaAlign"
echo "桌面入口：$appdir/metaalign.desktop"
case ":$PATH:" in
    *":$bindir:"*) ;;
    *) echo "提示：$bindir 不在 PATH 里，可直接用绝对路径启动，或先 export PATH=\"$bindir:\$PATH\"" ;;
esac
echo "卸载：rm -f $bindir/MetaAlign $appdir/metaalign.desktop $icondir/metaalign.png"
