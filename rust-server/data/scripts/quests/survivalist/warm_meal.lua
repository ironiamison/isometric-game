-- A Warm Meal Quest Script
-- Cooking tutorial quest from Camp Cook

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
        text = "Know how to fish? There's shrimp in the waters nearby. Catch a few and I'll show you how to cook 'em up proper. What do you say?",
        choices = {
            { id = "accept", text = "Teach me!" },
            { id = "decline", text = "Maybe later." },
            { id = "ask_more", text = "Tell me more about cooking." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:give_item("fishing_rod", 1)
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Here's a fishing rod. Head to the water and catch 3 shrimp. Then cook them at my fire pit here. Simple as that!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Cooking is one of the survivalist trades. Catch some fish, then use the fire pit to cook them up. Cooked food heals you better than raw. It's the foundation of staying alive out here."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "No rush. The fire's always burning. Come back when you're hungry!"
        })
    end
end

function show_progress_dialogue(ctx)
    local caught = ctx:get_objective_progress("catch_shrimp")
    local cooked = ctx:get_objective_progress("cook_shrimp")

    local text = string.format(
        "How's it going? You've caught %d of 3 shrimp and cooked %d of 3. Keep at it!",
        caught.current, cooked.current
    )

    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = text
    })
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "catch_shrimp" and new_count == 3 then
        ctx:show_notification("All shrimp caught! Now cook them at the fire pit.")
    elseif objective_id == "cook_shrimp" and new_count == 3 then
        ctx:show_notification("Shrimp cooked! Return to the Camp Cook.")
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "Nicely done! Nothing beats a warm meal. You're picking up the survivalist trade fast."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "The fire pit's always here if you need to cook something. I've got more to teach you if you're interested!"
    })
end
