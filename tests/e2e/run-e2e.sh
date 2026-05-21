#!/bin/bash
# E2E Test Runner for Cull
# Uses agent-browser + Chrome Beta against localhost:1420 with mock Tauri layer
#
# Prerequisites:
#   - Chrome Beta: --remote-debugging-port=9222
#   - Vite: npx vite dev --port 1420

CDP=9222
URL="http://localhost:1420"
SHOTS="/tmp/cull-e2e"
S="e2e$(LC_ALL=C tr -dc 'a-z0-9' < /dev/urandom | head -c 4)"
PASS=0; FAIL=0; ERRS=""

mkdir -p "$SHOTS"

G='\033[0;32m'; R='\033[0;31m'; B='\033[0;34m'; D='\033[2m'; N='\033[0m'

pass() { echo -e "${G}  ✓${N} $1"; PASS=$((PASS + 1)); }
fail() { echo -e "${R}  ✗${N} $1"; FAIL=$((FAIL + 1)); ERRS="${ERRS}\n  - $1"; }
log()  { echo -e "${B}[test]${N} $1"; }

ab() {
    local out
    out=$(agent-browser --cdp "$CDP" --session "$S" "$@" 2>&1) || true
    echo "$out"
}

get_page() {
    PAGE=$(ab snapshot -i)
}

has() {
    echo "$PAGE" | grep -qi "$1"
}

# Preflight
curl -s "http://localhost:$CDP/json/version" > /dev/null 2>&1 || { echo -e "${R}Chrome not on $CDP${N}"; exit 1; }
curl -s "$URL" > /dev/null 2>&1 || { echo -e "${R}App not on $URL${N}"; exit 1; }
command -v agent-browser &> /dev/null || { echo -e "${R}No agent-browser${N}"; exit 1; }

echo ""
echo -e "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"
echo -e "${B}  Cull E2E Tests ($S)${N}"
echo -e "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"
echo ""

# ── 1: App loads ──
log "App loads"
ab tab new "$URL" > /dev/null
sleep 3
get_page

has "Grid" && pass "Grid tab visible" || fail "No Grid tab"
has "All Images" && pass "All Images scope visible" || fail "No All Images"
has "/ to search" && pass "Search trigger visible" || fail "No search trigger"
has "Import Folder" && pass "Import Folder action visible" || fail "No Import Folder action"
has "AI MODELS" && pass "AI model controls visible" || fail "No AI model controls"
ab screenshot "$SHOTS/01-loaded.png" > /dev/null
echo ""

# ── 2: Search expands ──
log "Search expands"
SEARCH=$(echo "$PAGE" | grep '"/ to search"' | grep -oE 'e[0-9]+' | head -1)
if [ -n "$SEARCH" ]; then
    ab click "@$SEARCH" > /dev/null
    sleep 1
    get_page
    has "searchbox.*Search images" && pass "Search box appeared" || fail "No search box"
    has "Switch dictation language" && pass "Dictation language control present" || fail "No dictation language control"
    ab screenshot "$SHOTS/02-search.png" > /dev/null
else
    fail "Could not find search trigger"
fi
echo ""

# ── 3: Embedding Explorer ──
log "Embedding Explorer provider UI"
EMBED=$(echo "$PAGE" | grep 'Embeddings' | grep -oE 'e[0-9]+' | head -1)
if [ -n "$EMBED" ]; then
    ab click "@$EMBED" > /dev/null
    sleep 2
    get_page
    has "Visual embeddings" && pass "Embedding canvas landmark present" || fail "No embedding canvas landmark"
    has "CLIP" && pass "CLIP provider visible" || fail "No CLIP provider"
    has "DINOv2" && pass "DINOv2 provider visible" || fail "No DINOv2 provider"
    has "Gemini" && pass "Gemini provider visible" || fail "No Gemini provider"
    ab screenshot "$SHOTS/03-embeddings.png" > /dev/null
else
    fail "Could not find Embeddings tab"
fi
echo ""

# ── 4: Embedding controls ──
log "Embedding controls"
has "Map" && pass "Map interaction mode visible" || fail "No Map mode"
has "Stack" && pass "Stack interaction mode visible" || fail "No Stack mode"
has "Review" && pass "Review interaction mode visible" || fail "No Review mode"
has "Z PRESET" && pass "Z preset selector visible" || fail "No Z preset selector"
has "Download CLIP" && pass "Download action visible for missing local model" || fail "No Download CLIP action"
ab screenshot "$SHOTS/04-embedding-controls.png" > /dev/null
echo ""

# ── 5: Return to grid ──
log "Return to grid"
GRID=$(echo "$PAGE" | grep 'Grid' | grep -oE 'e[0-9]+' | head -1)
if [ -n "$GRID" ]; then
    ab click "@$GRID" > /dev/null
    sleep 2
    get_page
    has "No images loaded\|All Images" && pass "Grid view restored" || fail "Grid view did not restore"
    ab screenshot "$SHOTS/05-grid.png" > /dev/null
else
    fail "Could not find Grid tab"
fi
echo ""

# ── Cleanup & Results ──
ab tab close > /dev/null 2>&1 || true

echo -e "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"
echo -e "  ${G}Passed: $PASS${N}  ${R}Failed: $FAIL${N}"
[ -n "$ERRS" ] && echo -e "${R}  Failures:${N}$ERRS"
echo -e "  ${D}Screenshots: $SHOTS/${N}"
echo -e "${B}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"

exit $FAIL
