#!/usr/bin/env lua
-- =============================================================================
-- A Ghastly Contraption — Quest Script Test Harness
-- =============================================================================
--
-- Run: lua test_ghastly_contraption.lua
--
-- Mocks the server's ctx API and walks through every dialogue path in the
-- quest script, verifying the flow works correctly.

-- Track test results
local tests_run = 0
local tests_passed = 0
local tests_failed = 0
local current_test = ""

local function test(name, fn)
    current_test = name
    tests_run = tests_run + 1
    local ok, err = pcall(fn)
    if ok then
        tests_passed = tests_passed + 1
        print("  PASS  " .. name)
    else
        tests_failed = tests_failed + 1
        print("  FAIL  " .. name)
        print("        " .. tostring(err))
    end
end

local function assert_eq(a, b, msg)
    if a ~= b then
        error((msg or "assertion failed") .. ": expected " .. tostring(b) .. ", got " .. tostring(a), 2)
    end
end

local function assert_contains(str, substr, msg)
    if not string.find(str, substr, 1, true) then
        error((msg or "assertion failed") .. ": '" .. tostring(str) .. "' does not contain '" .. tostring(substr) .. "'", 2)
    end
end

local function assert_true(val, msg)
    if not val then
        error((msg or "assertion failed") .. ": expected true, got " .. tostring(val), 2)
    end
end

-- =============================================================================
-- Mock ctx object
-- =============================================================================

local function make_ctx(opts)
    opts = opts or {}
    local quest_state = opts.quest_state or "not_started"
    local objectives = opts.objectives or {}
    local choice_queue = opts.choices or {}
    local interacting_npc = opts.npc or ""
    local choice_idx = 0
    local flags = opts.flags or {}

    local ctx = {}
    ctx._dialogues = {}        -- all dialogues shown
    ctx._quest_accepted = false
    ctx._quest_completed = false
    ctx._items_given = {}
    ctx._notifications = {}
    ctx._unlocked_quests = {}
    ctx._completed_objectives = {}

    function ctx:get_quest_state()
        return quest_state
    end

    function ctx:get_interacting_npc()
        return interacting_npc
    end

    function ctx:get_objective_progress(obj_id)
        local obj = objectives[obj_id] or { current = 0, target = 1 }
        return { current = obj.current, target = obj.target }
    end

    function ctx:show_dialogue(opts)
        table.insert(self._dialogues, {
            speaker = opts.speaker,
            text = opts.text,
            choices = opts.choices
        })

        if opts.choices and #opts.choices > 0 then
            choice_idx = choice_idx + 1
            local choice = choice_queue[choice_idx]
            if not choice then
                error("No choice queued for dialogue #" .. choice_idx ..
                    " (speaker: " .. opts.speaker .. ", text: " .. opts.text:sub(1, 60) .. "...)")
            end
            return choice
        end
        return nil
    end

    function ctx:accept_quest()
        self._quest_accepted = true
    end

    function ctx:complete_quest()
        self._quest_completed = true
    end

    function ctx:give_item(item_id, count)
        table.insert(self._items_given, { id = item_id, count = count })
    end

    function ctx:show_notification(text)
        table.insert(self._notifications, text)
    end

    function ctx:unlock_quest(quest_id)
        table.insert(self._unlocked_quests, quest_id)
    end

    function ctx:get_flag(name)
        return flags[name]
    end

    function ctx:set_flag(name, value)
        flags[name] = value
    end

    function ctx:complete_objective(obj_id)
        table.insert(self._completed_objectives, obj_id)
    end

    return ctx
end

-- Helper: check if any dialogue contains a substring
local function any_dialogue_contains(ctx, substr)
    for _, d in ipairs(ctx._dialogues) do
        if string.find(d.text, substr, 1, true) then
            return true
        end
    end
    return false
end

-- Helper: check if any dialogue has a specific speaker
local function any_dialogue_from(ctx, speaker)
    for _, d in ipairs(ctx._dialogues) do
        if d.speaker == speaker then
            return true
        end
    end
    return false
end

-- Helper: get all items given
local function items_given(ctx)
    local result = {}
    for _, item in ipairs(ctx._items_given) do
        result[item.id] = (result[item.id] or 0) + item.count
    end
    return result
end

-- =============================================================================
-- Load the quest script
-- =============================================================================

print("")
print("Loading ghastly_contraption.lua...")
dofile("ghastly_contraption.lua")
print("Script loaded successfully.")
print("")

-- Verify functions exist
assert(type(on_interact) == "function", "on_interact must exist")
assert(type(on_objective_progress) == "function", "on_objective_progress must exist")
assert(type(on_use_item) == "function", "on_use_item must exist")
print("All expected functions found.")
print("")
print("Running tests...")
print(string.rep("-", 60))

-- =============================================================================
-- Test: Quest Offer — Accept
-- =============================================================================

test("Offer: accept quest", function()
    local ctx = make_ctx({
        quest_state = "not_started",
        choices = { "accept" },
    })
    on_interact(ctx)

    assert_true(ctx._quest_accepted, "Quest should be accepted")
    assert_true(any_dialogue_contains(ctx, "real person"), "Should greet player")
    assert_true(any_dialogue_contains(ctx, "auction"), "Should mention buying the house")
    assert_true(any_dialogue_contains(ctx, "candle mechanism"), "Should mention candle mechanism")
    assert_true(any_dialogue_contains(ctx, "tinderbox"), "Should mention tinderbox")
    assert_true(any_dialogue_contains(ctx, "bookshelves"), "Should point to bookshelves")
end)

-- =============================================================================
-- Test: Quest Offer — Decline
-- =============================================================================

test("Offer: decline quest", function()
    local ctx = make_ctx({
        quest_state = "not_started",
        choices = { "decline" },
    })
    on_interact(ctx)

    assert_true(not ctx._quest_accepted, "Quest should NOT be accepted")
    assert_true(any_dialogue_contains(ctx, "carriage wheels"), "Should show decline dialogue")
end)

-- =============================================================================
-- Test: Bookshelf Search — gives tinderbox
-- =============================================================================

test("Bookshelf search: gives tinderbox", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "haunted_bookshelf",
        objectives = {
            find_tinderbox = { current = 0, target = 1 },
            open_first_gate = { current = 0, target = 1 },
            talk_barnaby = { current = 0, target = 1 },
        },
    })
    on_interact(ctx)

    local given = items_given(ctx)
    assert_eq(given["tinderbox"], 1, "Should give 1 tinderbox")
    assert_true(any_dialogue_contains(ctx, "rummage"), "Should describe searching")
    assert_true(any_dialogue_contains(ctx, "metal box"), "Should find the tinderbox")
end)

-- =============================================================================
-- Test: Candle Interaction — Shows hint to use tinderbox
-- =============================================================================

test("Candle interaction: shows hint to use tinderbox", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "haunted_candles",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 0, target = 1 },
            talk_barnaby = { current = 0, target = 1 },
        },
    })
    on_interact(ctx)

    assert_true(any_dialogue_contains(ctx, "tinderbox"), "Should hint about tinderbox")
    assert_true(any_dialogue_contains(ctx, "candle stand"), "Should describe the candle")
end)

-- =============================================================================
-- Test: on_use_item — Candle Puzzle via use-item-on-entity
-- =============================================================================

test("on_use_item: correct candle order lights all 4", function()
    local flags = {}
    local ctx = make_ctx({
        quest_state = "in_progress",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 0, target = 1 },
        },
        flags = flags,
    })

    -- Light candle 1
    local r = on_use_item(ctx, "tinderbox", "haunted_candles", "candle_1")
    assert_true(r, "Should be handled")
    assert_true(#ctx._notifications > 0, "Should notify")
    assert_contains(ctx._notifications[#ctx._notifications], "skull candle", "Should name skull candle")
    assert_contains(ctx._notifications[#ctx._notifications], "green", "Should mention green flame")

    -- Light candle 2
    ctx._notifications = {}
    r = on_use_item(ctx, "tinderbox", "haunted_candles", "candle_2")
    assert_true(r, "Should be handled")
    assert_contains(ctx._notifications[#ctx._notifications], "tall taper", "Should name tall taper")

    -- Light candle 3
    ctx._notifications = {}
    r = on_use_item(ctx, "tinderbox", "haunted_candles", "candle_3")
    assert_true(r, "Should be handled")
    assert_contains(ctx._notifications[#ctx._notifications], "red candle", "Should name red candle")

    -- Light candle 4 (last one — gate opens)
    ctx._notifications = {}
    r = on_use_item(ctx, "tinderbox", "haunted_candles", "candle_4")
    assert_true(r, "Should be handled")
    assert_true(#ctx._completed_objectives > 0, "Should complete objective")
    assert_eq(ctx._completed_objectives[1], "open_first_gate", "Should complete open_first_gate")
end)

test("on_use_item: wrong candle order resets", function()
    local flags = {}
    local ctx = make_ctx({
        quest_state = "in_progress",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 0, target = 1 },
        },
        flags = flags,
    })

    -- Light candle 1 (correct)
    on_use_item(ctx, "tinderbox", "haunted_candles", "candle_1")
    assert_eq(flags["candles_lit"], "candle_1", "Should have candle_1 lit")

    -- Light candle 3 (WRONG — should be candle_2)
    ctx._notifications = {}
    on_use_item(ctx, "tinderbox", "haunted_candles", "candle_3")
    assert_eq(flags["candles_lit"], "", "Should reset all candles")
    assert_contains(ctx._notifications[1], "cold wind", "Should show failure")
end)

test("on_use_item: non-tinderbox returns false", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 0, target = 1 },
        },
    })
    local r = on_use_item(ctx, "bucket", "haunted_candles", "candle_1")
    assert_true(not r, "Should return false for non-tinderbox")
end)

test("on_use_item: non-candle entity returns false", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 0, target = 1 },
        },
    })
    local r = on_use_item(ctx, "tinderbox", "tree", "tree_1")
    assert_true(not r, "Should return false for non-candle")
end)

test("on_use_item: already lit candle shows message", function()
    local flags = { candles_lit = "candle_1" }
    local ctx = make_ctx({
        quest_state = "in_progress",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 0, target = 1 },
        },
        flags = flags,
    })
    on_use_item(ctx, "tinderbox", "haunted_candles", "candle_1")
    assert_contains(ctx._notifications[1], "already lit", "Should say already lit")
end)

-- =============================================================================
-- Test: Barnaby Interrogation — "good" answers + take key
-- =============================================================================

test("Barnaby: good answers (watch, bread, door) + take key", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "barnaby_ghost",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 1, target = 1 },
            talk_barnaby = { current = 0, target = 1 },
        },
        choices = { "watch", "bread", "door", "ask", "take" },
    })
    on_interact(ctx)

    assert_true(any_dialogue_from(ctx, "Barnaby"), "Should have Barnaby dialogue")
    assert_true(any_dialogue_contains(ctx, "breathe"), "Should ask about breathing")
    assert_true(any_dialogue_contains(ctx, "food"), "Should ask about food")
    assert_true(any_dialogue_contains(ctx, "walls"), "Should ask about walls")
    assert_true(any_dialogue_contains(ctx, "chest moves"), "Watch response")
    assert_true(any_dialogue_contains(ctx, "stew"), "Bread response")
    assert_true(any_dialogue_contains(ctx, "DOOR"), "Door response")
    assert_true(any_dialogue_contains(ctx, "believe you"), "Should be convinced")
    assert_true(any_dialogue_contains(ctx, "lucky charm"), "Should mention lucky charm")

    local given = items_given(ctx)
    assert_eq(given["basement_key"], 1, "Should give basement key")
end)

-- =============================================================================
-- Test: Barnaby — funny answers + gentle goodbye
-- =============================================================================

test("Barnaby: funny answers (do_you, ecto, can_you) + gentle", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "barnaby_ghost",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 1, target = 1 },
            talk_barnaby = { current = 0, target = 1 },
        },
        choices = { "do_you", "ecto", "can_you", "ask", "gentle" },
    })
    on_interact(ctx)

    assert_true(any_dialogue_contains(ctx, "perfectly alive"), "do_you response")
    assert_true(any_dialogue_contains(ctx, "disgusting"), "ecto response")
    assert_true(any_dialogue_contains(ctx, "Watch this"), "can_you response")
    assert_true(any_dialogue_contains(ctx, "floats through a wall"), "Wall float scene")
    assert_true(any_dialogue_contains(ctx, "explains a LOT"), "Gentle goodbye")

    local given = items_given(ctx)
    assert_eq(given["basement_key"], 1, "Should still give key with funny answers")
end)

-- =============================================================================
-- Test: Barnaby — suspicious answers + take key
-- =============================================================================

test("Barnaby: suspicious answers (obviously, none, yes) + take", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "barnaby_ghost",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 1, target = 1 },
            talk_barnaby = { current = 0, target = 1 },
        },
        choices = { "obviously", "none", "yes", "ask", "take" },
    })
    on_interact(ctx)

    assert_true(any_dialogue_contains(ctx, "EXACTLY what a ghost"), "Obviously response")
    assert_true(any_dialogue_contains(ctx, "Ghost confirmed"), "None response")
    assert_true(any_dialogue_contains(ctx, "I knew it"), "Yes response")
    -- Should still be convinced and give key
    assert_true(any_dialogue_contains(ctx, "believe you"), "Should still be convinced")

    local given = items_given(ctx)
    assert_eq(given["basement_key"], 1, "Should give key regardless of answers")
end)

-- =============================================================================
-- Test: Oddwick Waiting — before poltergeist fight
-- =============================================================================

test("Oddwick hint: tinderbox not found yet", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "prof_oddwick",
        objectives = {
            find_tinderbox = { current = 0, target = 1 },
            open_first_gate = { current = 0, target = 1 },
            talk_barnaby = { current = 0, target = 1 },
        },
    })
    on_interact(ctx)

    assert_true(any_dialogue_from(ctx, "Professor Oddwick"), "Should be Oddwick talking")
    assert_true(any_dialogue_contains(ctx, "bookshelves"), "Should hint about bookshelves")
    assert_eq(#ctx._items_given, 0, "Should NOT give tinderbox")
end)

test("Oddwick hint: candle puzzle not solved yet", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "prof_oddwick",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 0, target = 1 },
            talk_barnaby = { current = 0, target = 1 },
        },
    })
    on_interact(ctx)

    assert_true(any_dialogue_from(ctx, "Professor Oddwick"), "Should be Oddwick talking")
    assert_true(any_dialogue_contains(ctx, "tinderbox"), "Should acknowledge tinderbox found")
    assert_true(any_dialogue_contains(ctx, "skull"), "Should give (wrong) candle order hint")
end)

test("Barnaby post-key: already gave the key", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "barnaby_ghost",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 1, target = 1 },
            talk_barnaby = { current = 1, target = 1 },
            defeat_poltergeist = { current = 0, target = 1 },
            collect_ectoplasm = { current = 0, target = 1 },
            collect_coil = { current = 0, target = 1 },
        },
    })
    on_interact(ctx)

    assert_true(any_dialogue_from(ctx, "Barnaby"), "Should be Barnaby talking")
    assert_true(any_dialogue_contains(ctx, "basement"), "Should mention basement")
end)

test("Oddwick waiting: before poltergeist fight", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        npc = "prof_oddwick",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 1, target = 1 },
            talk_barnaby = { current = 1, target = 1 },
            defeat_poltergeist = { current = 0, target = 1 },
            collect_ectoplasm = { current = 0, target = 1 },
            collect_coil = { current = 0, target = 1 },
        },
    })
    on_interact(ctx)

    assert_true(
        any_dialogue_contains(ctx, "basement key") or any_dialogue_contains(ctx, "ruckus"),
        "Should tell player to go to basement"
    )
end)

-- =============================================================================
-- Test: Oddwick Waiting — poltergeist killed but items not collected
-- =============================================================================

test("Oddwick waiting: after fight, items not collected", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 1, target = 1 },
            talk_barnaby = { current = 1, target = 1 },
            defeat_poltergeist = { current = 1, target = 1 },
            collect_ectoplasm = { current = 0, target = 1 },
            collect_coil = { current = 0, target = 1 },
        },
    })
    on_interact(ctx)

    assert_true(
        any_dialogue_contains(ctx, "defeated") or any_dialogue_contains(ctx, "components") or any_dialogue_contains(ctx, "ectoplasm"),
        "Should ask about components"
    )
end)

-- =============================================================================
-- Test: Oddwick Waiting — all items collected
-- =============================================================================

test("Oddwick waiting: all items collected", function()
    local ctx = make_ctx({
        quest_state = "in_progress",
        objectives = {
            find_tinderbox = { current = 1, target = 1 },
            open_first_gate = { current = 1, target = 1 },
            talk_barnaby = { current = 1, target = 1 },
            defeat_poltergeist = { current = 1, target = 1 },
            collect_ectoplasm = { current = 1, target = 1 },
            collect_coil = { current = 1, target = 1 },
        },
    })
    on_interact(ctx)

    assert_true(
        any_dialogue_contains(ctx, "hand them over") or any_dialogue_contains(ctx, "assembly rig"),
        "Should be eager for components"
    )
end)

-- =============================================================================
-- Test: Build Sequence — explosion, Barnaby comment, success
-- =============================================================================

test("Build sequence: explosion → Barnaby → success → quest complete", function()
    local ctx = make_ctx({
        quest_state = "ready_to_complete",
    })
    on_interact(ctx)

    -- Verify the full build sequence
    assert_true(any_dialogue_contains(ctx, "You got them"), "Should celebrate getting components")
    assert_true(any_dialogue_contains(ctx, "ectoplasmic resonance"), "Should describe calibration")
    assert_true(any_dialogue_contains(ctx, "BANG"), "Should have explosion")
    assert_true(any_dialogue_contains(ctx, "wasn't supposed to happen"), "Oddwick reacts to explosion")
    assert_true(any_dialogue_contains(ctx, "supposed to be on fire"), "Barnaby asks about fire")
    assert_true(any_dialogue_contains(ctx, "No, Barnaby"), "Oddwick dismisses Barnaby")
    assert_true(any_dialogue_contains(ctx, "reverse the polarity"), "Second attempt")
    assert_true(any_dialogue_contains(ctx, "IT WORKS"), "Success!")
    assert_true(any_dialogue_contains(ctx, "60%"), "Mentions 60% recovery rate")
    assert_true(any_dialogue_contains(ctx, "recall my memories"), "Barnaby's memory joke")
    assert_true(any_dialogue_contains(ctx, "murdery"), "Final thank you")

    assert_true(ctx._quest_completed, "Quest should be completed")
end)

-- =============================================================================
-- Test: Post-Quest Dialogue — Upgrade Offer
-- =============================================================================

test("Post-quest: upgrade offer dialogue", function()
    local ctx = make_ctx({
        quest_state = "completed",
    })
    on_interact(ctx)

    assert_true(any_dialogue_contains(ctx, "ghost-hunter"), "Should greet returning player")
    assert_true(any_dialogue_contains(ctx, "72%"), "Should mention improved recovery rate")
    assert_true(any_dialogue_contains(ctx, "Ancient Fragments"), "Should mention upgrade materials")
    assert_true(any_dialogue_contains(ctx, "Improved Attractor"), "Should name the upgrade")
    assert_true(any_dialogue_contains(ctx, "Ranged level 50"), "Should mention level requirement")
end)

-- =============================================================================
-- Test: on_objective_progress — poltergeist kill notification
-- =============================================================================

test("Objective progress: poltergeist kill notification", function()
    local ctx = make_ctx({})
    on_objective_progress(ctx, "defeat_poltergeist", 1)

    assert_eq(#ctx._notifications, 1, "Should have 1 notification")
    assert_contains(ctx._notifications[1], "vanquished", "Should say vanquished")
end)

test("Objective progress: no notification for other objectives", function()
    local ctx = make_ctx({})
    on_objective_progress(ctx, "find_tinderbox", 1)
    assert_eq(#ctx._notifications, 0, "Should have no notifications for tinderbox")
end)

-- =============================================================================
-- Results
-- =============================================================================

print(string.rep("-", 60))
print("")
print(string.format("Results: %d passed, %d failed, %d total",
    tests_passed, tests_failed, tests_run))
print("")

if tests_failed > 0 then
    print("SOME TESTS FAILED!")
    os.exit(1)
else
    print("ALL TESTS PASSED!")
    os.exit(0)
end
