-- Adventurer Tasks III - progression tasks

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
        text = "Tier III benchmark: defeat 15 pigs, reach Farming level 8, and hold 900 gold. This is your early-game competency check.",
        choices = {
            { id = "accept", text = "I'll clear Tier III." },
            { id = "decline", text = "Not right now." },
            { id = "ask_tips", text = "How should I pace this?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Plan in loops: farm and gather while your combat route stays active. Keep your gold target protected until completion."
        })
    elseif choice == "ask_tips" then
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Longer tiers are won by pacing. Keep all three objectives moving every session."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Adventurer Guide",
            text = "Come back when you're ready for a full benchmark run."
        })
    end
end

function show_progress_dialogue(ctx)
    local pigs = ctx:get_objective_progress("kill_pigs")
    local farming = ctx:get_objective_progress("reach_farming_8")
    local gold = ctx:get_objective_progress("gather_gold_900")

    local text = string.format(
        "Tier III status:\n- Pigs defeated: %d/15\n- Farming level: %d/8\n- Gold: %d/900",
        pigs.current,
        farming.current,
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
        text = "Tier III complete. Your progression base is strong now - choose any specialization path and you'll advance efficiently."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Adventurer Guide",
        text = "Tier III is complete. Keep applying the same structured milestone approach."
    })
end
