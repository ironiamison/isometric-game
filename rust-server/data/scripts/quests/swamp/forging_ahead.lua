-- Forging Ahead Quest Script
-- Swamp Hermit Part 2: Gather materials to fix his sword

-- Called when player interacts with quest giver NPC
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

-- Show quest offer with choices
function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "Armour's sorted, but my sword... it snapped clean in half fighting a swamper. I need willow wood for a new handle, slime cores for tempering, and marshbloom for the finishing oil.",
        choices = {
            { id = "accept", text = "I'll gather the materials." },
            { id = "decline", text = "That's a lot of stuff." },
            { id = "ask_more", text = "How much do you need?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "I'll need 30 willow logs, 10 slime cores, and 5 marshbloom. It's a big ask, but I'll make it worth your while!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "30 willow logs for the handle and scabbard, 10 slime cores to temper the blade - nothing toughens steel like swamp slime - and 5 marshbloom to make finishing oil. Trust me, I know what I'm doing."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "Fair enough. I'll just sit here... swordless... defenceless... on a log."
        })
    end
end

-- Show progress dialogue
function show_progress_dialogue(ctx)
    local logs = ctx:get_objective_progress("collect_willow_logs")
    local cores = ctx:get_objective_progress("collect_slime_cores")
    local blooms = ctx:get_objective_progress("collect_marshbloom")

    local text = string.format(
        "That sword won't forge itself! I need willow logs (%d of 30), slime cores (%d of 10), and marshbloom (%d of 5). How's the gathering going?",
        logs.current, cores.current, blooms.current
    )

    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = text
    })
end

-- Called when an objective is updated
function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "collect_willow_logs" and new_count == 30 then
        ctx:show_notification("Willow logs collected!")
    elseif objective_id == "collect_slime_cores" and new_count == 10 then
        ctx:show_notification("Slime cores collected!")
    elseif objective_id == "collect_marshbloom" and new_count == 5 then
        ctx:show_notification("Marshbloom collected!")
    end
end

-- Complete the quest
function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "Now THAT'S what I call materials! Give me a moment to work... ha! A proper blade again. One more favour and I'll be ready to fight."
    })

    ctx:complete_quest()
end

-- Post-completion dialogue
function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "This sword is magnificent! But I still can't fight on an empty stomach..."
    })
end
