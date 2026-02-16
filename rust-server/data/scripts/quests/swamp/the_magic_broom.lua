-- The Magic Broom Quest Script
-- Witch Willow needs ingredients to craft a magic broom

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
        speaker = "Witch Willow",
        text = "My sister sent you? Oh, wonderful! I need to get back to her, but I can't leave without a magic broom. Could you gather the ingredients for me?",
        choices = {
            { id = "accept", text = "What do you need?" },
            { id = "decline", text = "Maybe later." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Witch Willow",
            text = "I need 5 willow logs for the handle, 3 marshbloom for the enchantment, and 2 slime cores for the binding spell. Bring them back and I'll craft the broom!"
        })
    else
        ctx:show_dialogue({
            speaker = "Witch Willow",
            text = "Oh... alright. I'll just be here then. Waiting. In the swamp. Alone."
        })
    end
end

-- Show progress dialogue with current counts
function show_progress_dialogue(ctx)
    local logs = ctx:get_objective_progress("collect_willow_logs")
    local blooms = ctx:get_objective_progress("collect_marshbloom")
    local cores = ctx:get_objective_progress("collect_slime_cores")

    local text = string.format(
        "Have you gathered all the ingredients? I need willow logs (%d of 5), marshbloom (%d of 3), and slime cores (%d of 2).",
        logs.current, blooms.current, cores.current
    )

    ctx:show_dialogue({
        speaker = "Witch Willow",
        text = text
    })
end

-- Called when an objective is updated
function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "collect_willow_logs" and new_count == 5 then
        ctx:show_notification("Willow logs collected!")
    elseif objective_id == "collect_marshbloom" and new_count == 3 then
        ctx:show_notification("Marshbloom collected!")
    elseif objective_id == "collect_slime_cores" and new_count == 2 then
        ctx:show_notification("Slime cores collected!")
    end
end

-- Complete the quest
function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Witch Willow",
        text = "You have everything! Let me work my magic..."
    })

    ctx:show_dialogue({
        speaker = "Witch Willow",
        text = "First, the willow logs... shaped and smoothed into a perfect handle. Now the marshbloom - ground into a fine powder for the enchantment..."
    })

    ctx:show_dialogue({
        speaker = "Witch Willow",
        text = "And finally, the slime cores to bind it all together. Stand back!"
    })

    ctx:show_dialogue({
        speaker = "Witch Willow",
        text = "There! The broom is enchanted and ready. Watch this - off it goes, flying straight to my sister! She'll have it in no time."
    })

    ctx:show_dialogue({
        speaker = "Witch Willow",
        text = "You've done us both a great service! My sister will be so happy. Tell her I'll be home soon - I just need to tidy up a few things here first."
    })

    ctx:complete_quest()
end

-- Post-completion dialogue
function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Witch Willow",
        text = "The broom is on its way to Hazel! Thank you again, dear. I'll make the journey home soon enough."
    })
end
