#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
package_root="$repository_root/FIPSDPackage"
install_root="${HOME:?}/.local/bin"
config_root="${HOME:?}/.config/fips-wind-tunnel"
app_path=""

while (($# > 0)); do
  case "$1" in
    --app)
      app_path="${2:?--app requires a path}"
      shift 2
      ;;
    -h|--help)
      echo "usage: scripts/install-mcp-server.sh [--app /path/to/FIPSD.app]"
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

swift build \
  --package-path "$package_root" \
  --configuration release \
  --product fips-wind-tunnel-mcp

binary_path="$(swift build \
  --package-path "$package_root" \
  --configuration release \
  --show-bin-path)/fips-wind-tunnel-mcp"
test -x "$binary_path"
install -d "$install_root"
install -m 0755 "$binary_path" "$install_root/fips-wind-tunnel-mcp"
install -d "$config_root"
printf '%s\n' "$repository_root" > "$config_root/workspace-path"
chmod 0600 "$config_root/workspace-path"

if [[ -n "$app_path" ]]; then
  if [[ ! -d "$app_path" ]]; then
    echo "app does not exist: $app_path" >&2
    exit 2
  fi
  printf '%s\n' "$app_path" > "$config_root/app-path"
  chmod 0600 "$config_root/app-path"
fi

echo "installed: $install_root/fips-wind-tunnel-mcp"
echo "configured workspace: $repository_root"
if [[ -n "$app_path" ]]; then
  echo "configured app: $app_path"
fi
