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
        text = "Tier II raises the bar: defeat 24 blue slimes and 16 crows, reach Woodcutting 8 and Combat 14, then build your gold reserve to 1,200.",
        choices = {
            { id = "accept", text = "Start Tier II." },
            { id = "decline", text = "Later." },
            { id = "ask_tips", text = "What's the efficient route?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Rotate combat and resource runs. Chop trees between fights to push woodcutting while keeping your income steady."
        })
    elseif choice == "ask_tips" then
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Avoid idling. If you're waiting on one objective, progress the others in parallel."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "No rush. Return when you're focused."
        })
    end
end

function show_progress_dialogue(ctx)
    local slimes = ctx:get_objective_progress("kill_blue_slimes")
    local crows = ctx:get_objective_progress("kill_crows")
    local woodcutting = ctx:get_objective_progress("reach_woodcutting_8")
    local combat = ctx:get_objective_progress("reach_combat_14")
    local gold = ctx:get_objective_progress("gather_gold_1200")

    local text = string.format(
        "Tier II status:\n- Blue slimes: %d/24\n- Crows: %d/16\n- Woodcutting level: %d/8\n- Combat level: %d/14\n- Gold: %d/1200",
        slimes.current,
        crows.current,
        woodcutting.current,
        combat.current,
        gold.current
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
