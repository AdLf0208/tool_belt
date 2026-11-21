#!/usr/bin/env python3
"""Craft Tool Belt's two carried recipes in a real crafting table.

Called by `tools/e2e.sh` with the open game's control socket. Uses the harness's
crafting grid (`ctl screen`: slots 100-108 row-major, result 200), gives the
ingredients, and asserts:

1. the belt's shaped recipe (`"sls" / "l l" / "lil"`): 2 string + 5 leather + 1
   iron ingot craft exactly one `toolbelt:belt`, and every ingredient is consumed;
2. the pouch's shaped recipe (`"sgs" / "l l" / "sls"`): 4 string + 3 leather + 1
   gold ingot craft exactly one `toolbelt:pouch`, and every ingredient is consumed;
3. control: the belt pattern without its iron ingot crafts nothing (the recipe is
   shaped, not "any leather and string"), so the assertions above can fail.

The table is replaced before every case, so a failed or untaken result cannot
leak into the next grid. Exits 1 with the observed result on the first mismatch.
"""
import json
import subprocess
import sys

BELT = "toolbelt:belt"
POUCH = "toolbelt:pouch"

# The jar's patterns, cell by cell: (row-major grid slot, item).
BELT_PATTERN = [
    (100, "minecraft:string"),
    (101, "minecraft:leather"),
    (102, "minecraft:string"),
    (103, "minecraft:leather"),
    (105, "minecraft:leather"),
    (106, "minecraft:leather"),
    (107, "minecraft:iron_ingot"),
    (108, "minecraft:leather"),
]
POUCH_PATTERN = [
    (100, "minecraft:string"),
    (101, "minecraft:gold_ingot"),
    (102, "minecraft:string"),
    (103, "minecraft:leather"),
    (105, "minecraft:leather"),
    (106, "minecraft:string"),
    (107, "minecraft:leather"),
    (108, "minecraft:string"),
]
# The belt pattern's ingredients, by item, and the same minus the iron ingot.
BELT_INGREDIENTS = [("minecraft:string", 2), ("minecraft:leather", 5), ("minecraft:iron_ingot", 1)]
BELT_CONTROL_INGREDIENTS = [("minecraft:string", 2), ("minecraft:leather", 5)]
POUCH_INGREDIENTS = [("minecraft:string", 4), ("minecraft:leather", 3), ("minecraft:gold_ingot", 1)]


def main():
    sock, ctl, x, y, z = sys.argv[1:6]

    def run(*args):
        out = subprocess.run([ctl, "--socket", sock, *args], text=True, capture_output=True)
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks=4):
        run("wait", str(ticks))

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item and slot.get("count", 0) > 0:
                return slot["slot"]
        raise SystemExit(f"missing ingredient {item} in the inventory")

    def result():
        """The result slot's stack, or None: an empty slot has `item: null`."""
        stack = [s for s in slots() if s["slot"] == 200]
        if not stack or not stack[0].get("item"):
            return None
        return stack[0]

    def place(item, targets):
        slot = find(item)
        run("click-slot", str(slot))
        wait(1)
        for target in targets:
            run("click-slot", str(target), "right")
            wait(1)
        # Put what is left of the stack back where it came from.
        run("click-slot", str(slot))
        wait(2)

    def setup(items):
        try:
            run("close")
        except SystemExit:
            pass
        wait(5)
        # Replace the table so its block entity, and therefore the crafting
        # grid, is fresh for this case.
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:crafting_table")
        wait(2)
        print("debug tp:", run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}"))
        wait(3)
        run("command", "clear @s")
        for item, count in items:
            run("give", item, str(count))
        wait(3)
        print("debug interact:", run("interact", x, y, z))
        wait(5)

    failures = []

    def craft(name, pattern, ingredients, expected, cells):
        setup(ingredients)
        by_item = {}
        for cell, item in pattern:
            by_item.setdefault(item, []).append(str(cell))
        for item, targets in by_item.items():
            place(item, targets)
        got = result()
        print(f"{name}: {json.dumps(got)}")
        if not got or got.get("item") != expected or got.get("count") != 1:
            failures.append(f"{name} crafted {got}, expected 1 {expected}")
            return
        # Shift-click the result: vanilla crafts and puts it in the inventory.
        run("click-slot", "200", "shift")
        wait(5)
        counts = run("inventory")["counts"]
        made = counts.get(expected, 0)
        left = sum(counts.get(item, 0) for item, _ in ingredients)
        print(f"{name}: {made} {expected}, {left} ingredients left")
        if made < 1:
            failures.append(f"{name}: the crafted {expected} reached the inventory as {made}")
        if left:
            failures.append(f"{name}: the recipe did not consume its ingredients ({left} left)")

    # 1. The belt's recipe.
    craft("belt recipe", BELT_PATTERN, BELT_INGREDIENTS, BELT, 8)

    # 2. The pouch's recipe.
    craft("pouch recipe", POUCH_PATTERN, POUCH_INGREDIENTS, POUCH, 6)

    # 3. Control: the belt pattern without the iron ingot.
    setup(BELT_CONTROL_INGREDIENTS)
    for item, targets in {
        "minecraft:string": ["100", "102"],
        "minecraft:leather": ["101", "103", "105", "106", "108"],
    }.items():
        place(item, targets)
    got = result()
    print(f"belt pattern without iron: {json.dumps(got)}")
    if got is not None:
        failures.append(f"the belt pattern without its iron ingot crafted {got['item']}")

    run("close")
    wait(3)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(
        "recipe PASS: belt and pouch craft one each from the jar's patterns and consume "
        "their ingredients; the belt pattern without its iron ingot stays empty"
    )


if __name__ == "__main__":
    main()
