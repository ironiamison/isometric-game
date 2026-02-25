-- Words of the Sealed - Quest 4 of The Awakening
-- Player gathers materials for the Resonance Lens

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
            text = "The Resonance Lens has revealed much. The Aetheri's warnings are clear - and terrifying."
        })
    end
end

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "I've partially translated the cistern symbols. They read: 'What sleeps beneath the sand must never wake. The seals hold. The seals must hold.'"
    })

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "The writing belongs to a lost civilization called the Aetheri. I need a Resonance Lens to read the rest, but it requires special materials."
    })

    local choice = ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "Can you gather 5 Refined Quartz from mining and 3 Construct Cores from the animated constructs?",
        choices = {
            { id = "accept", text = "I'll gather what you need." },
            { id = "decline", text = "Not right now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Refined Quartz can be found while mining. Construct Cores are rarer - you'll need to destroy more animated constructs. Bring everything to me."
        })
    else
        ctx:show_dialogue({
            speaker = "Archmage Yenara",
            text = "Every moment we delay, the inscriptions fade further. Please hurry."
        })
    end
end

function show_progress_dialogue(ctx)
    local quartz = ctx:get_objective_progress("collect_quartz")
    local cores = ctx:get_objective_progress("collect_cores")

    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = string.format("Materials: Refined Quartz %d/%d, Construct Cores %d/%d.", quartz.current, quartz.target, cores.current, cores.target)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "You have everything. Let me assemble the lens..."
    })
    ctx:show_dialogue({
        speaker = "Archmage Yenara",
        text = "There. The Resonance Lens is complete. With this, we can read what the Aetheri were desperately trying to tell us."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "collect_quartz" and new_count == 5 then
        ctx:show_notification("All Refined Quartz collected!")
    elseif objective_id == "collect_cores" and new_count == 3 then
        ctx:show_notification("All Construct Cores collected!")
    end
end
