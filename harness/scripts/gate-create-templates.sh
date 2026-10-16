#!/bin/bash
# WasenOffice gate 18 — Create-new templates (A01 T-A39, DD §5/Q7):
# every bundled Template blank must survive the REAL open pipeline — unit
# checks (naming seq, byte-exact copy, unknown-kind refuse, valid OOXML
# package with the root part each format requires) plus the x2t oracle: each
# blank converts through the shell's exact OpenBinary shape (template →
# Editor.bin), no app boot (~2 s, pattern gate 16). Script home of the gate
# command (verify.sh + dev-check.sh call this script directly — same shape as
# test-layout.sh, T-053 one-home rule).
set -u
cd "$(dirname "$0")/../.."
(cd src-tauri && cargo test --quiet untitled_)
