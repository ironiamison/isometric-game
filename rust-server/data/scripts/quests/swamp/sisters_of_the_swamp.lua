-- Sisters of the Swamp Quest Script
-- Witch Hazel sends you to find her sister Willow in the swamp village

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
        speaker = "Witch Hazel",
        text = "My sister Willow is stuck in the swamp village. She wandered off weeks ago and I haven't heard from her since. Could you find her and make sure she's alright?",
        choices = {
            { id = "accept", text = "I'll find her." },
            { id = "decline", text = "Not right now." },
            { id = "ask_more", text = "Where should I look?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Witch Hazel",
            text = "Follow the path deeper into the swamp. The village is hidden among the willows. Tell Willow I sent you!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Witch Hazel",
            text = "The swamp village is further south. You'll know you're close when the willow trees grow thick. Just find Willow and let her know I'm worried sick!"
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Witch Hazel",
            text = "I understand, dear. But please hurry back if you change your mind - I'm terribly worried about her."
        })
    end
end

-- Show progress dialogue
function show_progress_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Witch Hazel",
        text = "Have you found my sister yet? The village is deeper in the swamp. Please, she could be in trouble!"
    })
end

-- Called when an objective is updated
function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "find_witch_sister" then
        ctx:show_notification("You found Witch Willow! Return to Witch Hazel.")
    end
end

-- Complete the quest
function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Witch Hazel",
        text = "You found her! Oh, thank the stars. Is she well? What does she need?"
    })

    ctx:show_dialogue({
        speaker = "Witch Hazel",
        text = "A magic broom? Of course she does - always losing things, that one. Would you help her gather the ingredients? I'm sure she'll ask you herself."
    })

    ctx:complete_quest()
end

-- Post-completion dialogue
function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Witch Hazel",
        text = "Thank you for finding Willow. Have you helped her with that broom yet? She'll never get home without it!"
    })
end
