-- Axe to Grind Quest Script
-- Woodcutting tutorial quest from Lumberjack Pete

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
        speaker = "Lumberjack Pete",
        text = "So you want to learn woodcutting, eh? It's honest work - good for the arms and the soul. I can lend you my old bronze axe to get started. Chop down five oak trees nearby to prove you've got what it takes. What do you say?",
        choices = {
            { id = "accept", text = "I'm ready to chop!" },
            { id = "decline", text = "Maybe later." },
            { id = "ask_more", text = "How does woodcutting work?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:give_item("bronze_axe", 1)
        ctx:show_dialogue({
            speaker = "Lumberjack Pete",
            text = "Here's a bronze axe to get you started. Equip it from your inventory, face an oak tree, and keep swinging until it falls! The trees grow back, so don't worry about chopping 'em all down. Come back when you've felled five oaks!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Lumberjack Pete",
            text = "Woodcutting is simple. Equip an axe, stand next to a tree, face it, and swing. Keep swinging until the tree falls down - you'll get logs as you chop. Better axes chop faster. Different trees need different woodcutting levels - oak is perfect for beginners."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Lumberjack Pete",
            text = "No worries, friend. The trees aren't going anywhere. Come back when you're ready to swing an axe!"
        })
    end
end

function show_progress_dialogue(ctx)
    local progress = ctx:get_objective_progress("chop_oak_trees")

    local text = string.format(
        "How's the chopping going? You've felled %d of 5 oak trees so far. Keep at it!",
        progress.current
    )

    ctx:show_dialogue({
        speaker = "Lumberjack Pete",
        text = text
    })
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "chop_oak_trees" and new_count == 5 then
        ctx:show_notification("Chopped all 5 trees! Return to Lumberjack Pete.")
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Lumberjack Pete",
        text = "Five oak trees felled! You've got a natural talent for this. Here, take this iron axe - it's sharper and faster than that old bronze one. My shop's always open if you need better gear. Happy chopping!"
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Lumberjack Pete",
        text = "Good to see you, woodcutter! Check my shop if you need axes or want to sell your logs."
    })
end
