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

has "SMART\|5 Stars" && pass "Smart collections visible" || fail "No smart collections"
has "textbox.*landscape" && pass "Command bar present" || fail "No command bar"
has "All Images" && pass "All Images button present" || fail "No All Images"
ab screenshot "$SHOTS/01-loaded.png" > /dev/null
echo ""

# ── 2: NL parsing ──
log "Command bar parsing"
INPUT=$(echo "$PAGE" | grep 'textbox.*landscape' | grep -oE 'e[0-9]+' | head -1)
if [ -n "$INPUT" ]; then
    ab click "@$INPUT" > /dev/null
    sleep 1
    # Use native input setter + events to work with Svelte's bind:value
    ab eval "const el = document.querySelector('.command-input'); const nativeSet = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; nativeSet.call(el, 'portrait midjourney 4 stars'); el.dispatchEvent(new Event('input', {bubbles:true})); el.dispatchEvent(new Event('change', {bubbles:true}));" > /dev/null
    sleep 1
    # Dispatch keydown Enter
    ab eval "document.querySelector('.command-input').dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', code:'Enter', bubbles:true}));" > /dev/null
    sleep 2
    get_page
    has "Parsed\|parsed\|Orientation\|orientation\|rule" && pass "Rules shown" || fail "No rules after Enter"
    has "Apply" && pass "Apply button shown" || fail "No Apply button"
    ab screenshot "$SHOTS/02-parsed.png" > /dev/null
else
    fail "Could not find command input"
fi
echo ""

# ── 3: Apply ──
log "Apply filter"
APPLY=$(echo "$PAGE" | grep -i 'Apply' | grep -oE 'e[0-9]+' | head -1)
if [ -n "$APPLY" ]; then
    ab click "@$APPLY" > /dev/null
    sleep 3
    get_page
    has "images" && pass "Match count shown" || fail "No match count"
    has "Save Collection\|save-btn\|Save" && pass "Save button visible" || fail "No Save button"
    ab screenshot "$SHOTS/03-applied.png" > /dev/null
else
    fail "Could not find Apply button"
fi
echo ""

# ── 4: Save collection ──
log "Save collection"
SAVE=$(echo "$PAGE" | grep -i "Save Collection" | grep -oE 'e[0-9]+' | head -1)
if [ -n "$SAVE" ]; then
    # Use CSS selector — refs go stale between snapshot and click
    ab eval "document.querySelector('.save-btn').click();" > /dev/null
    sleep 2
    get_page
    has "NAME\|Name\|name-input\|Portrait\|Cancel" && pass "Name bar appeared" || fail "No name bar"
    ab screenshot "$SHOTS/04-naming.png" > /dev/null

    # Click Save confirm via CSS selector (refs go stale between snapshot and click)
    ab eval "document.querySelector('.save-confirm-btn').click();" > /dev/null
    sleep 3
    get_page
    has "Saved as\|saved\|saved-toast\|Portrait" && pass "Save confirmation shown" || fail "No save confirmation"
    ab screenshot "$SHOTS/05-saved.png" > /dev/null
else
    fail "Could not find Save button"
fi
echo ""

# ── 5: Click preset ──
log "Click preset collection"
sleep 3
get_page
PRESET=$(echo "$PAGE" | grep -i "5 Stars\|Picks\|Landscape" | grep -oE 'e[0-9]+' | head -1)
if [ -n "$PRESET" ]; then
    ab click "@$PRESET" > /dev/null
    sleep 2
    get_page
    pass "Clicked preset"
    ab screenshot "$SHOTS/06-preset.png" > /dev/null
else
    fail "No preset found to click"
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
