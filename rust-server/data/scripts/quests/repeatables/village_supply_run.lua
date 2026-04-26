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
        speaker = "Wise Man",
        text = "Discipline keeps villages standing. Bring me 8 oak logs, 6 copper ore, and 4 cooked shrimp for the emergency stores.",
        choices = {
            { id = "accept", text = "I'll gather the supplies." },
            { id = "ask_why", text = "Why these supplies?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Wise Man",
            text = "Wood for repairs, ore for tools, food for the watch. Practical bundles keep panic from turning into collapse."
        })
    elseif choice == "ask_why" then
        ctx:show_dialogue({
            speaker = "Wise Man",
            text = "Because fancy plans fail when a village has no timber, no metal, and no food. Bring the basics first."
        })
        return show_offer_dialogue(ctx)
    else
        return show_offer_dialogue(ctx)
    end
end

function show_progress_dialogue(ctx)
    local logs = ctx:get_objective_progress("collect_oak_logs")
    local ore = ctx:get_objective_progress("collect_copper_ore")
    local shrimp = ctx:get_objective_progress("collect_cooked_shrimp")

    local text = string.format(
        "Supply run status:\n- Oak logs: %d/%d\n- Copper ore: %d/%d\n- Cooked shrimp: %d/%d",
        logs.current, logs.target,
        ore.current, ore.target,
        shrimp.current, shrimp.target
    )

    ctx:show_dialogue({
        speaker = "Wise Man",
        text = text
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Wise Man",
        text = "Good. Efficient work, no waste, no excuses. That's how a settlement lasts."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Wise Man",
        text = "The stores can always use another disciplined run."
    })
end
