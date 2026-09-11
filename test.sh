#!/usr/bin/env bash
set -u

output_path=""
mode=""

while (($# > 0)); do
  case "$1" in
    --output_path)
      if (($# < 2)); then
        echo "Missing value for --output_path" >&2
        exit 2
      fi
      output_path="$2"
      shift 2
      ;;
    base|new)
      mode="$1"
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$output_path" || -z "$mode" ]]; then
  echo "Usage: $0 --output_path <path> base|new" >&2
  exit 2
fi

if [[ "$output_path" != /* ]]; then
  output_path="$PWD/$output_path"
fi

mkdir -p "$(dirname "$output_path")"
config_path="$(mktemp)"
trap 'rm -f "$config_path"' EXIT
printf '[profile.default.junit]\npath = "%s"\n' "$output_path" > "$config_path"

if [[ "$mode" == base ]]; then
  cargo nextest run -p wakaru-core --test un_esm_rule \
    --config-file "$config_path"
else
  cargo nextest run -p wakaru-core --test un_esm_rule_8c4d2e \
    --config-file "$config_path"
fi
