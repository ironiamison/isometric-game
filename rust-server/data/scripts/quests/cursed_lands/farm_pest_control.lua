-- Farm Pest Control Quest Script
-- Repeatable daily quest given by Farmer Grace

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
        speaker = "Farmer Grace",
        text = "Oh, thank goodness you're here! My pens are completely overrun - pigs squealing, worms burrowing everywhere, and the sheep have gone absolutely feral. I need someone tough to thin them out and bring me back some useful bits. What do you say?",
        choices = {
            { id = "accept", text = "I'll handle it." },
            { id = "decline", text = "Not right now." },
            { id = "ask_more", text = "What exactly do you need?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Farmer Grace",
            text = "You're a lifesaver! I need you to deal with 15 pigs, 10 sheep, and 10 worms. While you're at it, bring me back 3 worm segments, 3 piglets, and 3 scraps of leather. I'll pay you well for the trouble!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Farmer Grace",
            text = "I've got three pens full of trouble. The pig pen has at least 15 that need culling, there are 10 sheep gone wild, and 10 worms tearing up the ground. I also need 3 worm segments, 3 piglets, and 3 scraps of leather for my supplies."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Farmer Grace",
            text = "I understand, dear. But if you change your mind, I'll be right here - probably chasing pigs out of my garden."
        })
    end
end

-- Show progress dialogue with current counts
function show_progress_dialogue(ctx)
    local pigs = ctx:get_objective_progress("kill_pigs")
    local sheep = ctx:get_objective_progress("kill_sheep")
    local worms = ctx:get_objective_progress("kill_worms")
    local segments = ctx:get_objective_progress("collect_worm_segments")
    local piglets = ctx:get_objective_progress("collect_piglets")
    local leather = ctx:get_objective_progress("collect_scrap_leather")

    local text = string.format(
        "How's it going out there? Let's see... %d of 15 pigs, %d of 10 sheep, %d of 10 worms. " ..
        "And for materials: %d of 3 worm segments, %d of 3 piglets, %d of 3 scrap leather. Keep at it!",
        pigs.current, sheep.current, worms.current,
        segments.current, piglets.current, leather.current
    )

    ctx:show_dialogue({
        speaker = "Farmer Grace",
        text = text
    })
end

-- Called when an objective is updated
function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "kill_pigs" and new_count == 15 then
        ctx:show_notification("All pigs dealt with!")
    elseif objective_id == "kill_sheep" and new_count == 10 then
        ctx:show_notification("All sheep cleared out!")
    elseif objective_id == "kill_worms" and new_count == 10 then
        ctx:show_notification("All worms squashed!")
    elseif objective_id == "collect_worm_segments" and new_count == 3 then
        ctx:show_notification("Worm segments collected!")
    elseif objective_id == "collect_piglets" and new_count == 3 then
        ctx:show_notification("Piglets gathered!")
    elseif objective_id == "collect_scrap_leather" and new_count == 3 then
        ctx:show_notification("Scrap leather collected!")
    end
end

-- Complete the quest
function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Farmer Grace",
        text = "Would you look at that - every last one! The pens are manageable again. Here's 500 gold and a good chunk of experience for your trouble. You're welcome back anytime the critters get out of hand again!"
    })

    ctx:complete_quest()
end

-- Post-completion dialogue
function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Farmer Grace",
        text = "The pens are looking good for now, but you know how it is - those critters breed like... well, like pigs! Check back later if you want to help again."
    })
end
