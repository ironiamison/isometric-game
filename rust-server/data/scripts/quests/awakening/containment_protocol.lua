-- Containment Protocol - Quest 2 of The Awakening
-- Archmage Yenara sends player to destroy animated constructs and collect crystals

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "The dampening crystals are helping, but the source remains. We must go deeper."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "The disturbances have animated objects throughout the market - armor, crates, even furniture. They're attacking people. I need you to destroy them and bring me the dampening crystals inside them.",
        choices = {
            { id = "accept", text = "I'll handle it." },
            { id = "ask", text = "What are dampening crystals?" },
            { id = "decline", text = "Not now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Destroy the animated constructs and collect their crystals. I can use them to stabilize the ward matrix. Be careful - they're stronger than they look."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "When wild magic animates an object, it crystallizes around a core. Those crystals absorb magical energy - exactly what I need to calm these disturbances."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "People are in danger. Please reconsider."
        })
    end
end

function show_progress_dialogue(ctx)
    local kills = ctx:get_objective_progress("kill_constructs")
    local crystals = ctx:get_objective_progress("collect_crystals")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("Progress: %d/%d constructs destroyed, %d/%d crystals collected.", kills.current, kills.target, crystals.current, crystals.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "Excellent work. These crystals will help stabilize the area."
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "But this is only a stopgap. The source of these disturbances is something far deeper and far older. I need you to go beneath the city."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "kill_constructs" and new_count == 12 then
        ctx:show_notification("All constructs destroyed! Collect any remaining crystals.")
    end
end
