-- Rock Bottom Quest Script
-- Mining tutorial quest from Miner Mike

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
        speaker = "Miner Mike",
        text = "So you want to learn mining, eh? It's tough work - but the ore's worth every swing. I can lend you my old bronze pickaxe to get started. Mine five bronze rocks nearby to prove you've got what it takes. What do you say?",
        choices = {
            { id = "accept", text = "I'm ready to mine!" },
            { id = "decline", text = "Maybe later." },
            { id = "ask_more", text = "How does mining work?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:give_item("bronze_pickaxe", 1)
        ctx:show_dialogue({
            speaker = "Miner Mike",
            text = "Here's a bronze pickaxe to get you started. Equip it from your inventory, face a rock, and keep swinging until it crumbles! The rocks grow back, so don't worry about mining 'em all. Come back when you've mined five!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Miner Mike",
            text = "Mining is simple. Equip a pickaxe, stand next to a rock, face it, and swing away. Keep swinging until the rock crumbles - you'll get ore as you mine. Better pickaxes mine faster. Different rocks need different mining levels - bronze is perfect for beginners."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Miner Mike",
            text = "No worries, friend. The rocks aren't going anywhere. Come back when you're ready to swing a pickaxe!"
        })
    end
end

function show_progress_dialogue(ctx)
    local progress = ctx:get_objective_progress("mine_bronze_rocks")

    local text = string.format(
        "How's the mining going? You've crumbled %d of 5 bronze rocks so far. Keep at it!",
        progress.current
    )

    ctx:show_dialogue({
        speaker = "Miner Mike",
        text = text
    })
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "mine_bronze_rocks" and new_count == 5 then
        ctx:show_notification("Mined all 5 rocks! Return to Miner Mike.")
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Miner Mike",
        text = "Five bronze rocks smashed to bits! You've got a natural talent for this. Here, take this iron pickaxe - it's sturdier and faster than that old bronze one. My shop's always open if you need better gear. Happy mining!"
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Miner Mike",
        text = "Good to see you, miner! Check my shop if you need pickaxes or want to sell your ore."
    })
end
