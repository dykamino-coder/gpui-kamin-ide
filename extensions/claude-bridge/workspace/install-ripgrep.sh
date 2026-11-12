#!/bin/sh
# Release archives and checksums: https://github.com/BurntSushi/ripgrep/releases/tag/15.2.0
set -eu
version=15.2.0
case "$(dpkg --print-architecture)" in
  amd64)
    target=x86_64-unknown-linux-musl
    sha256=33e15bcf1624b25cdd2a55813a47a2f95dbe126268203e76aa6a585d1e7b149c
    ;;
  arm64)
    target=aarch64-unknown-linux-gnu
    sha256=a740b91c82eaf9914cfedd353572f2791cbe0162c84101ee0951058f4dcbc90d
    ;;
  *) echo 'Unsupported ripgrep architecture' >&2; exit 1 ;;
esac
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT
archive="ripgrep-$version-$target.tar.gz"
curl --fail --silent --show-error --location \
  "https://github.com/BurntSushi/ripgrep/releases/download/$version/$archive" \
  -o "$tmp_dir/$archive"
printf '%s  %s\n' "$sha256" "$tmp_dir/$archive" | sha256sum --check -
tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"
install -m 755 "$tmp_dir/ripgrep-$version-$target/rg" /usr/local/bin/rg
