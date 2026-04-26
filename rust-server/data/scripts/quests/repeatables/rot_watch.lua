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
        speaker = "Elder Mara",
        text = "The farms remain restless. I need another patrol: 10 corrupted pigs down, and 5 pieces of spoiled meat brought back for inspection.",
        choices = {
            { id = "accept", text = "I'll make the patrol." },
            { id = "ask_why", text = "Why collect the meat?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Elder Mara",
            text = "Thank you. We learn more about the corruption each time someone returns with fresh evidence."
        })
    elseif choice == "ask_why" then
        ctx:show_dialogue({
            speaker = "Elder Mara",
            text = "Because the sickness changes. If we stop watching it, one day it will outpace us."
        })
        return show_offer_dialogue(ctx)
    else
        return show_offer_dialogue(ctx)
    end
end

function show_progress_dialogue(ctx)
    local pigs = ctx:get_objective_progress("kill_corrupted_pigs")
    local meat = ctx:get_objective_progress("collect_spoiled_meat")

    local text = string.format(
        "Rot watch status:\n- Corrupted pigs slain: %d/%d\n- Spoiled meat recovered: %d/%d",
        pigs.current, pigs.target,
        meat.current, meat.target
    )

    ctx:show_dialogue({
        speaker = "Elder Mara",
        text = text
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Elder Mara",
        text = "You bought us another quiet stretch. In these lands, that matters more than you know."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Elder Mara",
        text = "The corruption will demand another patrol soon enough."
    })
end
