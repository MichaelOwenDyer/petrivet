#!/usr/bin/env bash
# Run petrivet-mcc on a single PNML file with the contest's environment
# variables already set. Mimics what BenchKit_head.sh does in the VM,
# minus the VM and the tar archive.
#
# Usage:
#   scripts/run.sh <path/to/model.pnml> <Examination>
#
#   <Examination> is any value of BK_EXAMINATION (StateSpace, OneSafe,
#   ReachabilityDeadlock, QuasiLiveness, StableMarking, Liveness,
#   UpperBounds, ReachabilityFireability, ReachabilityCardinality,
#   CTL/LTL Fireability/Cardinality).
#
# Examples:
#   scripts/run.sh ../../petrivet/tests/fixtures/philo.pnml StateSpace
#   scripts/run.sh ../../petrivet/tests/fixtures/mainframe0-04-04.pnml Liveness
#
# Optional environment overrides:
#   PETRIVET_MCC_BIN  path to the binary (default: cargo-built release)
#   BK_INPUT          override input "name" (default: pnml basename without .pnml)
set -euo pipefail

if [ $# -ne 2 ]; then
    echo "usage: $(basename "$0") <model.pnml> <Examination>" >&2
    exit 64
fi

PNML=$1
EXAMINATION=$2

if [ ! -f "$PNML" ]; then
    echo "error: PNML not found at $PNML" >&2
    exit 66
fi

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
CRATE_DIR=$SCRIPT_DIR/..
WORKSPACE_DIR=$CRATE_DIR/../..

BIN=${PETRIVET_MCC_BIN:-$WORKSPACE_DIR/target/release/petrivet-mcc}
if [ ! -x "$BIN" ]; then
    echo "building petrivet-mcc release binary..." >&2
    (cd "$WORKSPACE_DIR" && cargo build -p petrivet-mcc --release)
fi

BK_INPUT_NAME=${BK_INPUT:-$(basename "$PNML" .pnml)}

WORKDIR=$(mktemp -d)
trap 'rm -rf "$WORKDIR"' EXIT
cp "$PNML" "$WORKDIR/model.pnml"

if [[ "$BK_INPUT_NAME" == *"-COL-"* ]]; then
    echo "TRUE" > "$WORKDIR/iscolored"
else
    echo "FALSE" > "$WORKDIR/iscolored"
fi

cd "$WORKDIR"
env BK_TOOL=petrivet-mcc \
    BK_EXAMINATION="$EXAMINATION" \
    BK_INPUT="$BK_INPUT_NAME" \
    "$BIN"
