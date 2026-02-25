-- The Old Foundation - Quest 3 of The Awakening
-- Player explores the cisterns beneath New Aeven and discovers the ancient seal

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
            text = "That seal beneath the city haunts me. Whatever the Aetheri locked away, it's trying to break free."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "My research confirms it - New Aeven was built on top of something ancient. The cisterns beneath the city lead to older stonework. I need you to go down there and find the source of these disturbances.",
        choices = {
            { id = "accept", text = "I'll explore the cisterns." },
            { id = "decline", text = "Sounds dangerous. Maybe later." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Be careful down there. The magical energy is concentrated underground - there will be creatures drawn to it. Find the source and report back."
        })
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "It IS dangerous. But so is letting this continue unchecked."
        })
    end
end

function show_progress_dialogue(ctx)
    local wraiths = ctx:get_objective_progress("kill_seal_wraiths")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("How goes the exploration? Wraiths defeated: %d/%d.", wraiths.current, wraiths.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "A sealed wall with unknown symbols? And wraiths drawn to it?"
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "This is far more serious than I feared. Those symbols look ancient - older than anything in our archives. I must study them. Thank you for this discovery."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "find_seal" then
        ctx:show_notification("You've found the ancient seal! Report to Archmage Yenara.")
    end
end
