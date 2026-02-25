-- Tremors - Quest 5 of The Awakening
-- Sand Wraiths breach the city wall, player defends and investigates

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
            text = "The wall has been repaired, but the ley line beneath us is still active. The desert holds the answers we need."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "An earthquake just struck the city! The eastern wall has collapsed and creatures made of sand and shadow are pouring through!"
    })

    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "We need you at the breach NOW. Destroy the Sand Wraiths before they overrun the district!",
        choices = {
            { id = "accept", text = "I'm on my way!" },
            { id = "decline", text = "I can't right now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Get to the eastern wall and destroy those Sand Wraiths! Then inspect the damage - I need to understand how they got here."
        })
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "People will die if we don't act! Please reconsider!"
        })
    end
end

function show_progress_dialogue(ctx)
    local wraiths = ctx:get_objective_progress("kill_sand_wraiths")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("Sand Wraiths destroyed: %d/%d. Clear the breach!", wraiths.current, wraiths.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "A ley line connecting the desert directly to the seal beneath us... that explains everything."
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "Whatever the Aetheri sealed away, its influence stretches far further than I imagined. We must find a way to stop this at its source - in the desert itself."
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "I have a contact there - an Aetheri descendant named Kael. When the time comes, he may be our only hope."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "kill_sand_wraiths" and new_count == 15 then
        ctx:show_notification("All Sand Wraiths destroyed! Inspect the breach.")
    end
end
