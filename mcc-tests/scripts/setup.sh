#!/usr/bin/env bash
# setup.sh — prepare oracle verdicts and model PNML files for mcc-tests.
#
# Usage:
#   ./mcc-tests/scripts/setup.sh           # fetch oracle verdicts + download models
#   ./mcc-tests/scripts/setup.sh --oracle  # verdict files only (~50 MB, fast)
#   ./mcc-tests/scripts/setup.sh --models  # model PNML files only (large, slow)
#
# Outputs:
#   mcc-tests/oracle/models/<ModelId>/  — verdict .out files + model.pnml, co-located
#
# oracle.tar.gz is cached in mcc-tests/oracle/ after first download.
# Individual model archives are skipped if model.pnml already exists.
# --models requires --oracle to have been run first (verdict files drive the model ID list).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MCC_TESTS_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ORACLE_DIR="$MCC_TESTS_ROOT/oracle"
MODELS_DIR="$ORACLE_DIR/models"
ORACLE_URL="https://yanntm.github.io/pnmcc-models-2025/oracle.tar.gz"
ORACLE_CACHE="$ORACLE_DIR/oracle.tar.gz"
MODELS_BASE_URL="https://yanntm.github.io/pnmcc-models-2025/INPUTS"

do_oracle=true
do_models=true

case "${1:-}" in
  --oracle) do_models=false ;;
  --models) do_oracle=false ;;
  "")       ;;
  *) echo "Usage: $0 [--oracle|--models]" >&2; exit 1 ;;
esac

# Always ensure the models directory exists so the test harness has a root to walk.
mkdir -p "$MODELS_DIR"

# Download a URL to a destination file using curl or wget.
download() {
  local url="$1" dest="$2"
  if command -v curl &>/dev/null; then
    curl -fL --progress-bar -o "$dest" "$url"
  elif command -v wget &>/dev/null; then
    wget -q --show-progress -O "$dest" "$url"
  else
    echo "ERROR: neither curl nor wget is available." >&2
    exit 1
  fi
}

# ── Oracle verdict files ─────────────────────────────────────────────────────

if $do_oracle; then
  echo "Fetching oracle verdict files..."

  if [ ! -f "$ORACLE_CACHE" ]; then
    echo "  Downloading oracle.tar.gz..."
    download "$ORACLE_URL" "$ORACLE_CACHE"
  else
    echo "  Using cached $ORACLE_CACHE"
  fi

  echo "  Extracting and sorting verdict files into model directories..."
  ORACLE_TMP="$(mktemp -d)"
  tar -xzf "$ORACLE_CACHE" -C "$ORACLE_TMP" --strip-components=1

  while IFS= read -r f; do
    base="$(basename "$f" .out)"
    model_id="$(echo "$base" | sed 's/-[^-]*$//')"
    mkdir -p "$MODELS_DIR/$model_id"
    mv "$f" "$MODELS_DIR/$model_id/"
  done < <(find "$ORACLE_TMP" -maxdepth 1 -name "*.out")
  rm -rf "$ORACLE_TMP"

  COUNT=$(find "$MODELS_DIR" -mindepth 2 -maxdepth 2 -name "*.out" | wc -l | tr -d ' ')
  MODEL_COUNT=$(find "$MODELS_DIR" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')
  echo "  $COUNT verdict files across $MODEL_COUNT model directories in $MODELS_DIR"
fi

# ── Model PNML files ─────────────────────────────────────────────────────────

if $do_models; then
  if [ -z "$(find "$MODELS_DIR" -mindepth 2 -maxdepth 2 -name "*.out" -print -quit 2>/dev/null)" ]; then
    echo "ERROR: no verdict files found in $MODELS_DIR. Run '$0 --oracle' first." >&2
    exit 1
  fi

  echo "Downloading model PNML files..."

  # Derive unique model IDs from verdict filenames already sorted into subdirs.
  MODEL_IDS=$(
    find "$MODELS_DIR" -mindepth 2 -maxdepth 2 -name "*.out" \
      | sed 's|.*/||; s/\.out$//; s/-[^-]*$//' \
      | sort -u
  )

  TOTAL=$(echo "$MODEL_IDS" | wc -l | tr -d ' ')
  DONE=0
  SKIPPED=0

  while IFS= read -r model_id; do
    DONE=$((DONE + 1))
    model_dir="$MODELS_DIR/$model_id"

    if [ -f "$model_dir/model.pnml" ]; then
      SKIPPED=$((SKIPPED + 1))
      continue
    fi

    printf "  [%d/%d] %s\n" "$DONE" "$TOTAL" "$model_id"
    tmp="$(mktemp)"
    if download "$MODELS_BASE_URL/$model_id.tgz" "$tmp" 2>/dev/null; then
      mkdir -p "$model_dir"
      tar -xzf "$tmp" -C "$model_dir" --strip-components=1
    else
      echo "    WARNING: could not download $model_id" >&2
    fi
    rm -f "$tmp"
  done <<< "$MODEL_IDS"

  NEW=$((DONE - SKIPPED))
  echo "  $NEW downloaded, $SKIPPED already present."
fi

echo "Done."
