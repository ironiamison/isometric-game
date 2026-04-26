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
        speaker = "Camp Cook",
        text = "Training is over. Now I need output. Catch 6 shrimp, cook all 6, and bring me the finished meal.",
        choices = {
            { id = "accept", text = "I'll handle the supper." },
            { id = "ask_tip", text = "Any advice?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Good. Fish them yourself, cook them cleanly, and don't come back with burnt scraps."
        })
    elseif choice == "ask_tip" then
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Stay near the water and fire pit so you can loop fast. A survivalist wastes as little walking as possible."
        })
        return show_offer_dialogue(ctx)
    else
        return show_offer_dialogue(ctx)
    end
end

function show_progress_dialogue(ctx)
    local raw = ctx:get_objective_progress("catch_shrimp")
    local cooked = ctx:get_objective_progress("cook_shrimp")

    local text = string.format(
        "Campfire supper status:\n- Raw shrimp caught: %d/%d\n- Shrimp cooked: %d/%d",
        raw.current, raw.target,
        cooked.current, cooked.target
    )

    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = text
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "That's the standard I want. Fresh catch, properly cooked, delivered on time."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "If you want more work, I can always use another proper meal run."
    })
end
