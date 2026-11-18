#!/usr/bin/env bash
# In-game check for Tool Belt slice 1: the belt item, its storage and menu.
#
# One offscreen creative launch in The End on a floor, then:
#   1. the belt has two slots and starts empty;
#   2. `/toolbelt put` inserts a tool, `/toolbelt take` moves it back;
#   3. a stackable item (stone) is refused by the default rule, and a third
#      tool is refused when both slots are full;
#   4. `/toolbelt menu` opens the belt menu and it lists the belt's contents;
#   5. a click on a belt slot moves that stack to the player;
#   6. a right-click with the belt and an off-hand tool inserts the off-hand
#      stack (the click path);
#   7. the same world without --dev-mod registers nothing (the control).
#
# SELFTEST=1 asserts the impossible (the refused stone is in the belt), which
# must fail: it proves the storage assertions are not vacuous.
set -euo pipefail
trap 'echo "FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/toolbelt
CTL=${CTL:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/toolbelt-e2e.sock}
SELFTEST=${SELFTEST:-0}
# EMBEDDED=1 launches the native bundle alone (no --dev-mod): the content comes
# from `mixins/toolbelt/data/toolbelt-content.hmod` written out at start.
EMBEDDED=${EMBEDDED:-0}
DEV_ARGS=()
[ "$EMBEDDED" = 1 ] || DEV_ARGS=(--dev-mod "$PORT/content")

BELT=toolbelt:belt

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$PORT/content/horizon-mod.toml" ] || { echo "missing content mod: $PORT/content" >&2; exit 1; }

SCRATCH=$(mktemp -d "${E2E_TMP:-/tmp/opencode}"/toolbelt-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")"
rm -f "$SOCK"
cd "$SCRATCH"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; exit 1; }
say() { echo "== $* =="; }
count_item() { ctl inventory | jq -r --arg i "$1" '.counts[$i] // 0'; }
info() {
    ctl chat "/toolbelt info" >/dev/null
    ctl wait 3 >/dev/null
    ctl messages 6 | jq -r '.chat[]' | grep -m1 -E "^toolbelt:belt " || true
}
belt_has() { info | grep -q -- "$1"; }
belt_not_has() { ! info | grep -q -- "$1"; }
belt_text() { info | sed -n 's/.*contents=\[\(.*\)\]/\1/p'; }
belt_slots() { info | sed -n 's/.*slots=\([0-9]*\)\/.*/\1/p'; }
give_hotbar() { # item
    ctl command "item replace entity @s hotbar.0 with $1 1" >/dev/null
    ctl wait 3 >/dev/null
}
select_belt() { ctl select 1 >/dev/null; ctl wait 2 >/dev/null; }
open_menu() {
    ctl chat "/toolbelt menu" >/dev/null
    ctl wait 5 >/dev/null
    ctl wait-for screen container --timeout 10 >/dev/null \
        || fail "the belt menu did not open (screen=$(ctl state | jq -r '.screen // ""'))"
}

cleanup() {
    local status=$?
    echo "EXIT:$status"
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    rm -rf "$SCRATCH" || true
}
trap cleanup EXIT

say "launch (offscreen renderer, creative, content --dev-mod content/)"
"$CTL" --socket "$SOCK" launch \
    --game "$GAME" --world ToolBeltE2E --seed 20261109 --creative --offscreen --wait \
    --game-dir "$SCRATCH/game" --resources "$RES" "${DEV_ARGS[@]}"
ctl wait-ready 180
if [ "$EMBEDDED" = 1 ]; then
    cache=$(find "$SCRATCH" "$PORT" -maxdepth 5 -name "toolbelt-embedded-content.hmod" 2>/dev/null | head -1)
    if [ -n "$cache" ]; then
        cmp -s "$cache" "$PORT/content/toolbelt.hmod" \
            || fail "the embedded content differs from content/toolbelt.hmod"
        echo "  embedded content: $(wc -c <"$cache" | tr -d ' ') B, identical to content/toolbelt.hmod"
    else
        # The game consumes the written package into its own content cache; the
        # loader's startup line is then the proof that the bundle's copy loaded.
        grep -q 'content from 1 mod(s)' "${SOCK}.log" \
            || fail "the bundle's embedded content did not load (no 'content from 1 mod(s)' in ${SOCK}.log)"
        echo "  embedded content: loaded from the bundle (game content cache rebuilt at startup)"
    fi
fi
if grep -q "content mods not loaded" "${SOCK}.log" 2>/dev/null; then
    grep "content mods not loaded" "${SOCK}.log" >&2
    fail "the content mod did not load"
fi
ctl command "execute in minecraft:the_end run tp @s 0.5 64 0.5" >/dev/null
ctl wait 10
ctl command "execute in minecraft:the_end run fill -8 63 -8 8 63 8 minecraft:stone" >/dev/null
ctl command "execute in minecraft:the_end run fill -8 64 -8 8 72 8 minecraft:air" >/dev/null
ctl command "clear @s" >/dev/null
ctl wait 5

say "the belt has two slots and starts empty"
ctl command "item replace entity @s hotbar.0 with $BELT 1" >/dev/null
ctl give minecraft:iron_pickaxe 1 >/dev/null
ctl give minecraft:diamond_sword 1 >/dev/null
ctl give minecraft:iron_axe 1 >/dev/null
ctl give minecraft:stone 64 >/dev/null
ctl wait 5
select_belt
line=$(info)
echo "readout: $line"
echo "$line" | grep -q "size=2" || fail "a new belt is not size 2: $line"
[ "$(belt_slots)" = 0 ] || fail "a new belt is not empty: $line"

say "put and take a tool"
ctl chat "/toolbelt put minecraft:iron_pickaxe 1" >/dev/null
ctl wait 4
line=$(info)
echo "readout: $line"
echo "$line" | grep -q "minecraft:iron_pickaxe" || fail "the pickaxe was not inserted: $line"
[ "$(belt_slots)" = 1 ] || fail "the belt does not hold one stack: $line"
[ "$(count_item minecraft:iron_pickaxe)" = 0 ] || fail "the pickaxe stayed in the inventory"
ctl chat "/toolbelt take 0 1" >/dev/null
ctl wait 4
line=$(info)
echo "readout: $line"
echo "$line" | grep -q "minecraft:iron_pickaxe" && fail "the pickaxe was not taken: $line"
[ "$(count_item minecraft:iron_pickaxe)" = 1 ] || fail "the pickaxe did not return to the inventory"

say "control: a stackable item is refused by the default rule"
before=$(info)
ctl chat "/toolbelt put minecraft:stone 64" >/dev/null
ctl wait 4
line=$(info)
echo "readout: $line"
echo "$line" | grep -q "minecraft:stone" && fail "a stackable stone went into the belt: $line"
[ "$(count_item minecraft:stone)" = 64 ] || fail "the refused stone left the inventory"
echo "$before" | grep -q "contents=\[\]" || fail "the belt was not empty before the stone control: $before"

say "capacity: two slots, a third tool is refused"
ctl chat "/toolbelt put minecraft:iron_pickaxe 1" >/dev/null
ctl wait 3
ctl chat "/toolbelt put minecraft:diamond_sword 1" >/dev/null
ctl wait 3
line=$(info)
echo "readout: $line"
[ "$(belt_slots)" = 2 ] || fail "the belt does not hold two stacks: $line"
ctl chat "/toolbelt put minecraft:iron_axe 1" >/dev/null
ctl wait 4
line=$(info)
echo "readout: $line"
[ "$(belt_slots)" = 2 ] || fail "a full belt took a third tool: $line"
[ "$(count_item minecraft:iron_axe)" = 1 ] || fail "the refused axe left the inventory"

say "the menu shows the held belt's contents"
open_menu
menu=$(ctl menu)
echo "$menu"
[ "$(echo "$menu" | jq '[.slots[] | select(.slot >= 300 and .stack.item == "minecraft:iron_pickaxe")] | length')" = 1 ] \
    || fail "the menu does not show the pickaxe in a belt slot: $menu"
[ "$(echo "$menu" | jq '[.slots[] | select(.slot >= 300 and .stack.item == "minecraft:diamond_sword")] | length')" = 1 ] \
    || fail "the menu does not show the sword in a belt slot: $menu"

say "a click on a belt slot moves that stack to the player"
ctl click-slot 300 >/dev/null || fail "cannot click belt menu slot 300"
ctl wait 4
menu=$(ctl menu)
echo "$menu"
[ "$(echo "$menu" | jq '[.slots[] | select(.slot >= 300 and .stack.item == "minecraft:iron_pickaxe")] | length')" = 0 ] \
    || fail "the clicked pickaxe is still in a belt slot: $menu"
[ "$(count_item minecraft:iron_pickaxe)" = 1 ] || fail "the pickaxe did not reach the player"
line=$(info)
echo "readout: $line"
[ "$(belt_slots)" = 1 ] || fail "the belt did not lose the clicked stack: $line"
ctl close >/dev/null
ctl wait 4

say "a right-click with an off-hand tool inserts it"
ctl command "item replace entity @s hotbar.0 with $BELT 1" >/dev/null
ctl command "item replace entity @s weapon.offhand with minecraft:iron_shovel 1" >/dev/null
ctl wait 4
select_belt
ctl wait 2
ctl use-item >/dev/null
ctl wait 5
line=$(info)
echo "readout: $line"
echo "$line" | grep -q "minecraft:iron_shovel" || fail "the off-hand shovel was not inserted: $line"
off=$(ctl inventory | jq -r '.offhand.item // "none"')
echo "offhand after: $off"
[ "$off" != "minecraft:iron_shovel" ] || fail "the off-hand still holds the inserted shovel"

if [ "$SELFTEST" = 1 ]; then
    say "SELFTEST: the refused stone must be in the belt (it is not)"
    info | grep -q "minecraft:stone" \
        || fail "SELFTEST: the no-stackable control did not put the stone in the belt (the check is alive)"
fi

say "an extraction that does not fit keeps the tool in the belt"
# Fill every inventory slot and the off hand, then ask for a belt slot back. The
# old extraction removed the stack from the belt before giving it, so a full
# inventory destroyed the tool; the belt must be unchanged instead.
ctl command "clear @s minecraft:stone" >/dev/null
ctl wait 3
before=$(info)
echo "readout before: $before"
for _ in $(seq 1 36); do ctl command "give @s minecraft:stone 64" >/dev/null; done
ctl command "item replace entity @s weapon.offhand with minecraft:stone 64" >/dev/null
ctl wait 4
ctl chat "/toolbelt take 0 1" >/dev/null
ctl wait 4
after=$(info)
echo "readout after:  $after"
[ "$before" = "$after" ] || fail "a full inventory changed the belt: '$before' -> '$after'"
ctl command "clear @s minecraft:stone" >/dev/null
ctl wait 3

say "control: the same world without --dev-mod registers nothing"
ctl quit >/dev/null 2>&1 || true
sleep 1
rm -f "$SOCK"
# The bundle now embeds its own content (the AE2 pattern). Block that one file
# with a directory of the same name under the game's cache root, so the helper
# can neither read nor write it and falls back to `Continue`; the rest of the
# cache (the game's own mod-content staging) is untouched.
rm -f "$SCRATCH/game/.horizon-cache/toolbelt-embedded-content.hmod"
mkdir -p "$SCRATCH/game/.horizon-cache/toolbelt-embedded-content.hmod"
"$CTL" --socket "$SOCK" launch \
    --game "$GAME" --world ToolBeltNoMod --seed 20261109 --creative --offscreen --wait \
    --game-dir "$SCRATCH/game" --resources "$RES"
ctl wait-ready 180
ctl give "$BELT" 1 >/dev/null 2>&1 || true
ctl wait 3
[ "$(count_item "$BELT")" = 0 ] || fail "without --dev-mod the belt item was known"
echo "without --dev-mod: give $BELT added nothing"
ctl chat "/toolbelt info" >/dev/null
ctl wait 3
if ctl messages 6 | jq -r '.chat[]' | grep -q "^toolbelt:belt "; then
    fail "without --dev-mod the /toolbelt command answered"
fi
echo "without --dev-mod: /toolbelt answered nothing"

echo "PASS: the belt stores tools, refuses stackables and overflow, shows them in its menu, gives one back on a click, takes an off-hand tool on a right-click, keeps a tool in the belt when the inventory cannot take it, and registers nothing without --dev-mod"
