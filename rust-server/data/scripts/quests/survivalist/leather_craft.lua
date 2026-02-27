-- Leather and Craft Quest Script
-- Leatherworking tutorial quest from Camp Cook

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
        text = "Last lesson - leatherworking. I've got an animal hide and some wool for you. Spin the wool into thread, tan the hide into leather, then craft yourself some gloves at a workbench. What do you say?",
        choices = {
            { id = "accept", text = "Let's do it!" },
            { id = "decline", text = "Maybe later." },
            { id = "ask_more", text = "Tell me more about leatherworking." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:give_item("scrap_leather", 1)
        ctx:give_item("wool", 1)
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Here's an animal hide and some wool. Spin the wool into thread at a workbench, tan the hide into leather, then craft leather gloves. The workshop has plenty of workbenches."
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "Leatherworking turns raw animal hides into useful gear. You'll also need thread spun from wool to stitch things together. Tan the hide, spin some thread, then craft all sorts - gloves, boots, armor. It's essential for any survivalist."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Camp Cook",
            text = "No worries. The hides will keep. Come back when you're ready to learn."
        })
    end
end

function show_progress_dialogue(ctx)
    local hide = ctx:get_objective_progress("get_hide")
    local leather = ctx:get_objective_progress("tan_leather")
    local gloves = ctx:get_objective_progress("craft_gloves")

    local text
    if hide.current < 1 then
        text = "You'll need an animal hide first. I gave you one - check your inventory, or hunt some creatures for more."
    elseif leather.current < 1 then
        text = "Got the hide? Good. Now take it to a workbench in the workshop and tan it into leather."
    elseif gloves.current < 1 then
        text = "Nice, you've got leather! Now use the workbench to craft leather gloves."
    else
        text = "How's the leatherworking going? Remember - tan the hide first, then craft the gloves."
    end

    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = text
    })
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "get_hide" and new_count == 1 then
        ctx:show_notification("Got the hide! Now tan it into leather at a workbench.")
    elseif objective_id == "tan_leather" and new_count == 1 then
        ctx:show_notification("Leather tanned! Now craft leather gloves at the workbench.")
    elseif objective_id == "craft_gloves" and new_count == 1 then
        ctx:show_notification("Leather gloves crafted! Return to the Camp Cook.")
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "Leather gloves! You've learned cooking, fletching, and leatherworking. You're a proper survivalist now. Here's some extra hides to keep practicing."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Camp Cook",
        text = "You've mastered the survivalist basics! Keep honing your skills - there's always more to learn in the wild."
    })
end
