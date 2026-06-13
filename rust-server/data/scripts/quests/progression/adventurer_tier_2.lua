-- Adventurer Tasks II - progression tasks

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        return show_completed_dialogue(ctx)
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Adventurer Guide",
        text = "Tier II raises the bar: defeat 10 blue slimes and 12 worms, reach Woodcutting 8, Mining 8, Smithing 8, and Combat 14, then build your gold reserve to 1,200.",
        choices = {
            { id = "accept", text = "Start Tier II." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Rotate combat and resource runs. Mine ore and smelt bars between fights. Chop trees to push woodcutting while keeping your income steady."
        })
    else
        return show_offer_dialogue(ctx)
    end
end

function show_progress_dialogue(ctx)
    local slimes = ctx:get_objective_progress("kill_blue_slimes")
    local worms = ctx:get_objective_progress("kill_worms")
    local woodcutting = ctx:get_objective_progress("reach_woodcutting_8")
    local mining = ctx:get_objective_progress("reach_mining_8")
    local smithing = ctx:get_objective_progress("reach_smithing_8")
    local combat = ctx:get_objective_progress("reach_combat_14")
    local gold = ctx:get_objective_progress("gather_gold_1200")

    local text = string.format(
        "Tier II status:\n- Blue slimes: %d/%d\n- Worms: %d/%d\n- Woodcutting level: %d/%d\n- Mining level: %d/%d\n- Smithing level: %d/%d\n- Combat level: %d/%d\n- Gold: %d/%d",
        slimes.current, slimes.target,
        worms.current, worms.target,
        woodcutting.current, woodcutting.target,
        mining.current, mining.target,
        smithing.current, smithing.target,
        combat.current, combat.target,
        gold.current, gold.target
    )

    ctx:show_dialogue({
        speaker = "Adventurer Guide",
        text = text
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Adventurer Guide",
        text = "Tier II complete. You're building range and consistency. Tier III will test execution under longer goals."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Adventurer Guide",
        text = "Tier II is complete. Take Tier III when you want the next challenge."
    })
end
