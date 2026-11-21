#!/usr/bin/env bash
# In-game check for Tool Belt slice 1: the belt item, its storage and menu.
#
# A launcher-built game carrying this flat SDK package (the belt item, its
# content and its Rust all live in one `.hmod`), one offscreen creative launch in
# The End on a floor, then:
#   1. the belt has two slots and starts empty;
#   2. `/toolbelt put` inserts a tool, `/toolbelt take` moves it back;
#   3. a stackable item (stone) is refused by the default rule, and a third
#      tool is refused when both slots are full;
#   4. `/toolbelt menu` opens the belt menu and it lists the belt's contents;
#   5. a click on a belt slot moves that stack to the player;
#   6. a right-click with the belt and an off-hand tool inserts the off-hand
#      stack (the click path);
#   7. a shulker box (a container) is refused, so no belt-box-belt nesting;
#   8. the belt's and the pouch's carried recipes craft in a real crafting table,
#      consuming their ingredients, and the belt pattern without its iron ingot
#      stays empty (`tools/craft_check.py`);
#   9. a pouch takes a belt from two slots to nine in a real anvil, one pouch and
#      the jar's level cost per step, keeping the stored tool; leather and a
#      nine-slot belt produce no result (`tools/anvil_check.py`);
#  10. with CONTROL_GAME, the same world on a game without the package knows no
#      belt and no `/toolbelt`.
#
# SELFTEST=1 asserts the impossible (the refused stone is in the belt), which
# must fail: it proves the storage assertions are not vacuous.
set -euo pipefail
trap 'echo "FAILED at line $LINENO: $BASH_COMMAND" >&2' ERR

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/toolbelt
# The launcher's own control client (`launcher-game.sh env <mod>` exports CTL).
# It must be the pin's, so there is no fallback to another build's binary.
CTL=${CTL:-}
GAME=${GAME:-}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/toolbelt-e2e.sock}
SELFTEST=${SELFTEST:-0}

BELT=toolbelt:belt

[ -n "$CTL" ] || {
    echo "missing CTL: run eval \"\$(ports/tools/launcher-game.sh env toolbelt)\"" >&2
    exit 1
}
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -n "$GAME" ] || {
    echo "missing GAME: point it at a launcher-built horizon-client that carries toolbelt" >&2
    echo "  horizon-launcher native-build $PORT --instance <i> --trust-native-code" >&2
    exit 1
}

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

say "launch (offscreen renderer, creative, the package compiled into the game)"
"$CTL" --socket "$SOCK" launch \
    --game "$GAME" --world ToolBeltE2E --seed 20261109 --creative --offscreen --wait \
    --game-dir "$SCRATCH/game" --resources "$RES"
ctl wait-ready 180
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

say "/toolbelt help lists its five usages"
ctl chat "/toolbelt help" >/dev/null
ctl wait 4
help_lines=$(ctl messages 8 | jq -r '.chat[]' | grep -c '^/toolbelt ' || true)
echo "help lists: $help_lines usages"
[ "$help_lines" = 5 ] || fail "/toolbelt help did not list five usages (got $help_lines)"

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

say "control: a container-bearing stack (a shulker box) is refused"
ctl command "clear @s minecraft:stone" >/dev/null
ctl wait 3
ctl give minecraft:shulker_box 1 >/dev/null
ctl wait 4
before=$(info)
ctl chat "/toolbelt put minecraft:shulker_box 1" >/dev/null
ctl wait 4
line=$(info)
echo "readout: $line"
echo "$line" | grep -q "minecraft:shulker_box" && fail "a shulker box went into the belt: $line"
[ "$(count_item minecraft:shulker_box)" = 1 ] || fail "the refused shulker box left the inventory"
[ "$before" = "$line" ] || fail "the refused shulker box changed the belt: '$before' -> '$line'"

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

say "the carried recipes craft in a real crafting table"
ctl command "clear @s" >/dev/null
ctl command "execute in minecraft:the_end run setblock 5 64 5 minecraft:crafting_table" >/dev/null
ctl wait 3
python3 "$PORT/tools/craft_check.py" "$SOCK" "$CTL" 5 64 5 \
    || fail "the carried recipes did not craft"

say "a pouch upgrades a belt to nine slots at an anvil, at the jar's level costs"
ctl command "execute in minecraft:the_end run setblock 2 64 0 minecraft:anvil" >/dev/null
ctl wait 3
python3 "$PORT/tools/anvil_check.py" "$SOCK" "$CTL" 2 64 0 \
    || fail "the anvil upgrade chain did not hold"

if [ -n "$CONTROL_GAME" ]; then
    say "control: the same world on a game without the package"
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    rm -f "$SOCK"
    "$CTL" --socket "$SOCK" launch \
        --game "$CONTROL_GAME" --world ToolBeltNoMod --seed 20261109 --creative --offscreen --wait \
        --game-dir "$SCRATCH/game" --resources "$RES"
    ctl wait-ready 180
    ctl give "$BELT" 1 >/dev/null 2>&1 || true
    ctl wait 3
    [ "$(count_item "$BELT")" = 0 ] || fail "the control game knew the belt item"
    echo "  without the package: give $BELT added nothing"
    ctl chat "/toolbelt info" >/dev/null
    ctl wait 3
    if ctl messages 6 | jq -r '.chat[]' | grep -q "^toolbelt:belt "; then
        fail "the control game's /toolbelt answered"
    fi
    echo "  without the package: /toolbelt answered nothing"
fi

echo "PASS: the belt stores tools, refuses stackables, overflow and a container-bearing shulker box, shows them in its menu, gives one back on a click, takes an off-hand tool on a right-click, keeps a tool in the belt when the inventory cannot take it, its belt and pouch recipes craft in a table, and a pouch takes it from two to nine anvil slots at the jar's level costs"
