-- Adventurer Tasks I - milestone style progression tasks

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
        text = "No fluff, just progress. Tier I has three milestones: defeat 8 crows, reach Combat level 8, and build up 150 gold. Complete all three, then report back.",
        choices = {
            { id = "accept", text = "Assign me Tier I." },
            { id = "ask_tips", text = "Any route advice?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Good. Crows are nearby, and your combat level rises with real fights. Keep the gold you earn; don't spend below your target."
        })
    elseif choice == "ask_tips" then
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Loop between easy monsters and loot pickup. You'll train combat and stack gold at the same time."
        })
        return show_offer_dialogue(ctx)
    else
        return show_offer_dialogue(ctx)
    end
end

function show_progress_dialogue(ctx)
    local crows = ctx:get_objective_progress("kill_crows")
    local combat = ctx:get_objective_progress("reach_combat_8")
    local gold = ctx:get_objective_progress("gather_gold_150")

    local text = string.format(
        "Tier I status:\n- Crows: %d/8\n- Combat level: %d/8\n- Gold: %d/150",
        crows.current,
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
        text = "Tier I complete. Good baseline discipline. Claim your reward and step into Tier II when ready."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Adventurer Guide",
        text = "Tier I is logged as complete. Tier II is now your next benchmark."
    })
end
