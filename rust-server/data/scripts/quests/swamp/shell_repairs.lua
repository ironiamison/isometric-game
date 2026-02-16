-- Shell Repairs Quest Script
-- Swamp Hermit Part 1: Fix his armour with snail shells

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
        text = "Look at this armour - full of holes! The snails around here have the sturdiest shells I've ever seen. Bring me 10 and I can patch myself up.",
        choices = {
            { id = "accept", text = "I'll get your shells." },
            { id = "decline", text = "Find them yourself." },
            { id = "ask_more", text = "Why are you on a log?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "Smash those snails and bring me their shells. Should be plenty crawling around the swamp!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "I'm a warrior, not a swimmer! Sat down to rest and the swamp rose around me. Now I'm stuck here until I fix my gear. So... about those shells?"
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "Charming. I'll just sit here on my log then. Not like I'm going anywhere."
        })
    end
end

-- Show progress dialogue
function show_progress_dialogue(ctx)
    local shells = ctx:get_objective_progress("collect_snail_shells")

    local text = string.format(
        "Still collecting shells? I've got %d of 10 so far. I'm not going anywhere - literally stuck on this log.",
        shells.current
    )

    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = text
    })
end

-- Called when an objective is updated
function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "collect_snail_shells" and new_count == 10 then
        ctx:show_notification("All snail shells collected! Return to the Swamp Hermit.")
    end
end

-- Complete the quest
function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "Beautiful shells! Give me a moment... there, good as new. Well, good enough. Now about my sword..."
    })

    ctx:complete_quest()
end

-- Post-completion dialogue
function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "Armour's holding up nicely! But my sword is another story entirely..."
    })
end
