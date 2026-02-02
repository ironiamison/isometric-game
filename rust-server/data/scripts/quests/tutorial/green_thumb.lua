-- Green Thumb Quest Script
-- Farming tutorial quest from Farmer Barley

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
        speaker = "Farmer Barley",
        text = "You look like you could use a lesson in farming! I've got some potato seeds here. Plant them in the allotment patches nearby, wait for them to grow, and harvest the spuds. What do you say?",
        choices = {
            { id = "accept", text = "I'd love to learn!" },
            { id = "decline", text = "Maybe later." },
            { id = "ask_more", text = "Tell me more about farming." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:give_item("potato_seed", 3)
        ctx:show_dialogue({
            speaker = "Farmer Barley",
            text = "Here are three potato seeds. Drag them from your inventory onto an empty allotment patch to plant. They'll take about five minutes to grow. Come back when you've harvested three potatoes!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Farmer Barley",
            text = "Farming is simple. Plant a seed in an allotment patch, wait for it to grow through four stages, then harvest when it's ready. Different crops need different farming levels, but potatoes are perfect for beginners."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Farmer Barley",
            text = "No worries. The patches aren't going anywhere. Come back when you're ready to get your hands dirty!"
        })
    end
end

function show_progress_dialogue(ctx)
    local planted = ctx:get_objective_progress("plant_potatoes")
    local harvested = ctx:get_objective_progress("harvest_potatoes")

    local text = string.format(
        "How's the farming going? You've planted %d of 3 seeds and harvested %d of 3 potatoes.",
        planted.current, harvested.current
    )

    ctx:show_dialogue({
        speaker = "Farmer Barley",
        text = text
    })
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "plant_potatoes" and new_count == 3 then
        ctx:show_notification("All seeds planted! Now wait for them to grow.")
    elseif objective_id == "harvest_potatoes" and new_count == 3 then
        ctx:show_notification("Potatoes harvested! Return to Farmer Barley.")
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Farmer Barley",
        text = "Beautiful spuds! You're a natural farmer. Here, take these seeds - onions and tomatoes will serve you well. My shop's open to you now. Happy farming!"
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Farmer Barley",
        text = "Good to see you! The allotment patches are always open. Check my shop if you need more seeds."
    })
end
