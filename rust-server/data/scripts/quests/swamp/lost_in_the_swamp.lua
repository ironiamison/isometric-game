-- Lost in the Swamp Quest Script
-- A lost adventurer needs you to find his map

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
        speaker = "Lost Adventurer",
        text = "I dropped my map somewhere deeper in the swamp. Without it, I'm completely lost! Could you search for it?",
        choices = {
            { id = "accept", text = "I'll look for it." },
            { id = "decline", text = "Good luck with that." },
            { id = "ask_more", text = "Where did you lose it?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Lost Adventurer",
            text = "I think I lost it somewhere to the south, near some old willow trees. If you find it, come straight back!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Lost Adventurer",
            text = "I was heading south through the swamp when a swamper jumped me. I dropped everything and ran! The map should still be there, near some old willow trees."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Lost Adventurer",
            text = "Can't blame you. This swamp gives me the creeps too."
        })
    end
end

-- Show progress dialogue
function show_progress_dialogue(ctx)
    local map_found = ctx:get_objective_progress("find_the_map")

    if map_found.current > 0 then
        ctx:show_dialogue({
            speaker = "Lost Adventurer",
            text = "You found it? Brilliant! Let me see... yes, that's my map! Thank you so much!"
        })
    else
        ctx:show_dialogue({
            speaker = "Lost Adventurer",
            text = "Any luck finding my map? I'm going in circles without it. Try looking to the south, near the willow trees."
        })
    end
end

-- Called when an objective is updated
function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "find_the_map" then
        ctx:show_notification("You found the lost map! Return to the Lost Adventurer.")
    end
end

-- Complete the quest
function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Lost Adventurer",
        text = "You found it! I knew it had to be out there somewhere. Here, take this gold - you've saved me days of wandering!"
    })

    ctx:show_dialogue({
        speaker = "Lost Adventurer",
        text = "Between you and me, this map shows some interesting spots deeper in the swamp. There's an old ruin marked here that I was trying to find before everything went wrong. Maybe you'll have better luck than I did!"
    })

    ctx:complete_quest()
end

-- Post-completion dialogue
function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Lost Adventurer",
        text = "Thanks again for finding my map! I'm working up the courage to explore that ruin. One day..."
    })
end
