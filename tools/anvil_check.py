#!/usr/bin/env python3
"""Upgrade a Tool Belt with pouches in a real anvil, one slot per pouch.

Called by `tools/e2e.sh` with the open game's control socket. It seeds a belt
(with one tool inside), a stack of pouches and a pickaxe, switches the player to
survival with 200 levels, and then, for every step from two slots to nine:

1. opens a **fresh** anvil (the vanilla anvil wears out on use) with a hotbar
   slot that is not the belt selected, so the belt's own right-click does not
   open the belt menu instead;
2. shift-clicks the belt into the left input and one pouch into the right one;
3. asserts the anvil's result is `toolbelt:belt`, its level cost is the jar's
   `XP_COSTS` step for that size, and the result's `toolbelt:belt_size` is one
   larger with the stored tool still in slot 0 (the server's own output, read
   from the anvil's result slot before it is taken);
4. shift-clicks the result out, then asserts the tool belt now carries the new
   size, exactly one pouch was consumed, and the level dropped by the cost.

Controls that make every assertion able to fail:

- a belt with **leather** instead of a pouch produces no result and costs
  nothing (the upgrade needs the pouch, not any second item);
- a **nine-slot** belt with a pouch produces no result, consumes no pouch and
  costs no level (the table has no tenth step).

Exits 1 with the observed state on the first mismatch.
"""
import json
import re
import subprocess
import sys

BELT = "toolbelt:belt"
POUCH = "toolbelt:pouch"
TOOL = "minecraft:iron_pickaxe"
# `ToolBelt.XP_COSTS`: 2->3, 3->4, ... 8->9.
COSTS = {2: 3, 3: 5, 4: 8, 5: 12, 6: 15, 7: 20, 8: 30}


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

    def menu():
        return run("menu")

    def level():
        return run("state")["player"]["xp"]["level"]

    def slots():
        return {s["slot"]: s.get("stack") or {} for s in menu().get("slots", [])}

    def screen_slots():
        """The open screen's slots as drawn: these carry `components` (SNBT)."""
        return {s["slot"]: s for s in run("screen").get("slots", []) if s.get("item")}

    def find(item):
        for slot, stack in slots().items():
            if stack.get("item") == item and slot < 36:
                return slot
        raise SystemExit(f"missing {item} in the inventory: {slots()}")

    def count(item):
        return sum(
            stack.get("count", 0)
            for stack in slots().values()
            if stack.get("item") == item
        )

    def components(slot):
        stack = screen_slots().get(slot) or {}
        return stack.get("components") or ""

    def size_in(components_text):
        found = re.search(r'toolbelt:belt_size"\s*:\s*(\d+)', components_text)
        return int(found.group(1)) if found else None

    def fresh_anvil():
        run("close")
        wait(3)
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:anvil")
        wait(2)
        # Stand beside it: the crafting leg (and the previous step) may have
        # left the player elsewhere, and `interact` refuses a block out of reach.
        run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}")
        wait(2)
        # An empty hotbar slot is selected: a belt in hand would open the belt
        # menu on the right click instead of the anvil.
        run("select", "5")
        wait(1)
        opened = run("interact", x, y, z)
        wait(4)
        if opened.get("screen") not in ("container", "station"):
            raise SystemExit(f"the anvil did not open: {opened}")
        if menu().get("station") != "Anvil":
            raise SystemExit(f"the open menu is not the anvil station: {menu()}")

    def offer(left, right):
        """Shift-click `left` and `right` into the anvil, then the result."""
        run("click-slot", str(find(left)), "shift")
        wait(3)
        run("click-slot", str(find(right)), "shift")
        wait(3)
        info = menu().get("info") or {}
        return info.get("result"), info.get("anvil_cost")

    failures = []

    def check(good, message):
        if not good:
            failures.append(message)
            print(f"FAIL: {message}", file=sys.stderr)

    # Seed: a belt with a tool in it, eight pouches, and the pickaxe to store.
    run("command", "clear @s")
    run("command", f"item replace entity @s hotbar.0 with {BELT} 1")
    run("command", f"item replace entity @s hotbar.1 with {POUCH} 8")
    run("command", f"item replace entity @s hotbar.4 with {TOOL} 1")
    wait(3)
    run("select", "1")
    wait(1)
    run("chat", f"/toolbelt put {TOOL} 1")
    wait(4)
    run("chat", "/toolbelt info")
    wait(4)
    readout = "\n".join(run("messages", "4").get("chat", []))
    print(f"seeded belt: {readout.splitlines()[0] if readout else '<no readout>'}")
    check("size=2" in readout, f"the seeded belt is not two slots: {readout}")
    check(TOOL in readout, f"the seeded belt does not hold the tool: {readout}")
    run("command", "gamemode survival @s")
    run("command", "xp set @s 200 levels")
    wait(3)

    # Control 1: leather is not the upgrade material.
    fresh_anvil()
    run("command", f"give @s minecraft:leather 1")
    wait(3)
    result, cost = offer(BELT, "minecraft:leather")
    print(f"belt + leather -> result {result!r}, cost {cost!r}")
    check(result is None, f"a belt with leather produced {result!r}")
    check(not cost, f"a belt with leather costs {cost!r}")
    run("close")
    wait(3)
    run("command", "clear @s minecraft:leather")

    # The upgrade chain: eight pouches take the belt from two slots to nine.
    belt_slot = None
    for size in range(2, 9):
        from_size = size
        fresh_anvil()
        result, cost = offer(BELT, POUCH)
        before = level()
        print(f"step {from_size}->{from_size + 1}: result {result!r}, cost {cost!r}, level {before}")
        check(result == f"{BELT} x1", f"the anvil's result is {result!r}, expected one belt")
        check(cost == COSTS[from_size], f"the cost is {cost!r}, expected {COSTS[from_size]}")
        offered = components(650)
        check(
            size_in(offered) == from_size + 1,
            f"the result's belt size is {size_in(offered)}, expected {from_size + 1}: {offered}",
        )
        check(TOOL in offered, f"the result lost the stored tool: {offered}")
        pouches_before = count(POUCH)
        run("click-slot", "650", "shift")
        wait(4)
        after = level()
        pouches_after = count(POUCH)
        belt_slot = find(BELT)
        taken = components(belt_slot)
        print(
            f"  taken: belt at slot {belt_slot} size {size_in(taken)}, "
            f"pouches {pouches_before}->{pouches_after}, level {before}->{after}"
        )
        check(after == before - COSTS[from_size], f"the level went {before}->{after}, expected -{COSTS[from_size]}")
        check(
            size_in(taken) == from_size + 1,
            f"the taken belt reads size {size_in(taken)}, expected {from_size + 1}: {taken}",
        )
        check(TOOL in taken, f"the taken belt lost the stored tool: {taken}")
        check(pouches_after == pouches_before - 1, f"pouches went {pouches_before}->{pouches_after}, expected -1")
        run("close")
        wait(3)

    # Control 2: a nine-slot belt has no tenth step.
    fresh_anvil()
    before = level()
    pouches_before = count(POUCH)
    result, cost = offer(BELT, POUCH)
    after = level()
    pouches_after = count(POUCH)
    print(f"nine-slot belt + pouch -> result {result!r}, cost {cost!r}, pouches {pouches_before}->{pouches_after}")
    check(result is None, f"a nine-slot belt produced {result!r}")
    check(not cost, f"a nine-slot belt costs {cost!r}")
    check(after == before, f"the refused upgrade changed the level {before}->{after}")
    check(pouches_after == pouches_before, f"the refused upgrade consumed a pouch ({pouches_before}->{pouches_after})")
    run("close")
    wait(3)

    # The user-facing readout agrees with the components: put the belt in the
    # hotbar from the inventory screen first, wherever the anvil returned it.
    run("close")
    wait(2)
    run("open-inventory")
    wait(3)
    where = None
    for slot, entry in screen_slots().items():
        if entry.get("item") == BELT:
            where = slot
            break
    if where is not None and where >= 9:
        run("click-slot", str(where))
        wait(2)
        run("click-slot", "0")
        wait(2)
        where = 0
    run("close")
    wait(2)
    run("select", str((where or 0) + 1))
    wait(2)
    run("chat", "/toolbelt info")
    wait(4)
    readout = "\n".join(run("messages", "4").get("chat", []))
    print(f"final readout: {readout.splitlines()[0] if readout else '<no readout>'}")
    check("size=9" in readout, f"the upgraded belt does not read nine slots: {readout}")
    check(TOOL in readout, f"the upgraded belt does not hold the tool: {readout}")

    if failures:
        raise SystemExit(1)
    print(
        "anvil PASS: a pouch takes a belt from two to nine slots at the jar's seven level "
        "steps, one pouch each, with the stored tool kept; leather and a nine-slot belt "
        "produce no result"
    )


if __name__ == "__main__":
    main()
