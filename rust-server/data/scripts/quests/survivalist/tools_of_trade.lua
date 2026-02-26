-- Tools of the Trade Quest Script
-- Fletching tutorial quest from Camp Cook

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
        text = "Ready for the next lesson? Head to the Oakshore workshop - there's a knife in there you can use. Grab it and fletch some arrow shafts. What do you say?",
        choices = {
            { id = "accept", text = "I'm ready!" },
            { id = "decline", text = "Maybe later." },
            { id = "ask_more", text = "Tell me more about fletching." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:give_item("oak_log", 1)
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "The workshop entrance is just south of here. Pick up the knife, then right-click it in your inventory and choose 'Fletch' to cut arrow shafts from logs. Here's a log to get you started."
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Fletching is the art of making arrows. Right-click a knife in your inventory and choose 'Fletch' to cut logs into arrow shafts, then combine them with arrowheads. A good supply of arrows can save your life."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Take your time. The workshop isn't going anywhere."
        })
    end
end

function show_progress_dialogue(ctx)
    local knife = ctx:get_objective_progress("get_knife")
    local shafts = ctx:get_objective_progress("fletch_shafts")

    local text
    if knife.current < 1 then
        text = "Found that knife yet? The workshop is south of the fire pit. Look for the entrance."
    else
        text = string.format(
            "Good, you've got the knife! You've fletched %d of 15 arrow shafts so far. Keep going!",
            shafts.current
        )
    end

    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = text
    })
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "get_knife" and new_count == 1 then
        ctx:show_notification("Got the knife! Now use it with logs to fletch arrow shafts.")
    elseif objective_id == "fletch_shafts" and new_count == 15 then
        ctx:show_notification("Arrow shafts complete! Return to the Camp Cook.")
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "Arrow shafts! You're a natural. Here, take some bronze arrowheads - combine them with your shafts to make proper arrows."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "Keep fletching arrows when you can. A survivalist always keeps a good supply on hand."
    })
end
