#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fips_commit="80c956a6fdb85dde1450969a21891c1158e43267"
expected="$repository_root/fixtures/codecs/fips-80c956a.json"
mode="${1:---check}"

temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT
fips_checkout="$temporary_directory/fips"
mkdir -p "$fips_checkout"

candidate="${FIPSD_FIPS_SOURCE:-}"
if [[ -z "$candidate" && -d "$repository_root/../fips/.git" ]]; then
  candidate="$repository_root/../fips"
fi

if [[ -n "$candidate" ]] && git -C "$candidate" cat-file -e "$fips_commit^{commit}" 2>/dev/null; then
  git -C "$candidate" archive "$fips_commit" | tar -x -C "$fips_checkout"
else
  git clone --quiet https://github.com/jmcorgan/fips.git "$fips_checkout"
  git -C "$fips_checkout" checkout --quiet "$fips_commit"
fi

perl -pi -e 's/^pub\(crate\) mod proto;/pub mod proto;/' "$fips_checkout/src/lib.rs"
perl -pi -e 's/^pub\(crate\) mod /pub mod /' "$fips_checkout/src/proto/mod.rs"
for module_name in bloom fmp fsp lookup routing stp; do
  perl -pi -e 's/^(?:pub\(crate\) )?mod wire;/pub mod wire;/' "$fips_checkout/src/proto/$module_name/mod.rs"
done
cp "$repository_root/tools/fips-codec-probe.rs" "$fips_checkout/src/bin/fipsd-codec-probe.rs"

actual="$temporary_directory/actual.json"
(cd "$fips_checkout" && CARGO_TARGET_DIR="$repository_root/target/fips-codec-probe" cargo run --quiet --bin fipsd-codec-probe) > "$actual"

case "$mode" in
  --check)
    diff -u <(jq -S . "$expected") <(jq -S . "$actual")
    ;;
  --update)
    jq -S . "$actual" > "$expected"
    ;;
  --print)
    jq -S . "$actual"
    ;;
  *)
    echo "usage: $0 [--check|--update|--print]" >&2
    exit 2
    ;;
esac
