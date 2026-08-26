#!/bin/sh

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
IOS_DIR=$(dirname "$SCRIPT_DIR")
DESTINATION="$IOS_DIR/PythonResources"
ARCHIVE="Python-3.13-iOS-support.b14.tar.gz"
DOWNLOAD_URL="https://github.com/beeware/Python-Apple-support/releases/download/3.13-b14/$ARCHIVE"

if [ -d "$DESTINATION/Python.xcframework" ]; then
  echo "Python.xcframework 已存在，无需重复下载。"
  exit 0
fi

WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT INT TERM

echo "正在下载 BeeWare Python 3.13-b14 iOS 运行时…"
curl -fL "$DOWNLOAD_URL" -o "$WORK_DIR/$ARCHIVE"
tar -xzf "$WORK_DIR/$ARCHIVE" -C "$WORK_DIR"
cp -R "$WORK_DIR/Python.xcframework" "$DESTINATION/"
echo "已安装到 $DESTINATION/Python.xcframework"
