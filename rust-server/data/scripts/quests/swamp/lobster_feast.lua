-- Lobster Feast Quest Script
-- Swamp Hermit Part 3: Fish 50 lobsters to fuel him up

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
        speaker = "Swamp Hermit",
        text = "Armour? Fixed. Sword? Sharp. But I haven't eaten properly in weeks! The lobsters in the swamp village pond are magnificent. Bring me 50 and I'll have the energy to start slaying again.",
        choices = {
            { id = "accept", text = "50 lobsters? Sure." },
            { id = "decline", text = "That's ridiculous." },
            { id = "ask_more", text = "Why 50?!" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "50 lobsters! I know it's a lot, but a warrior needs fuel. The fishing spots in the swamp should have plenty."
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "Have you SEEN me? I'm a big lad! And I've been sitting on this log for weeks burning through my reserves. 50 lobsters is the bare minimum for a proper feast. Well... it's also my favourite number."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Swamp Hermit",
            text = "Ridiculous?! A man needs to eat! Fine, I'll just waste away here. On my log. Hungry."
        })
    end
end

-- Show progress dialogue
function show_progress_dialogue(ctx)
    local lobsters = ctx:get_objective_progress("collect_lobsters")

    local text
    if lobsters.current < 10 then
        text = string.format(
            "Only %d lobsters? My stomach is rumbling just thinking about them. Keep fishing!",
            lobsters.current
        )
    elseif lobsters.current < 25 then
        text = string.format(
            "%d lobsters so far! Getting there. I can almost taste them...",
            lobsters.current
        )
    elseif lobsters.current < 40 then
        text = string.format(
            "%d lobsters! Now we're talking. Just a few more and it's feast time!",
            lobsters.current
        )
    else
        text = string.format(
            "%d of 50! So close I can smell them cooking! Don't stop now!",
            lobsters.current
        )
    end

    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = text
    })
end

-- Called when an objective is updated
function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "collect_lobsters" then
        if new_count == 25 then
            ctx:show_notification("Halfway there! 25 of 50 lobsters.")
        elseif new_count == 50 then
            ctx:show_notification("50 lobsters! Return to the Swamp Hermit for the feast!")
        end
    end
end

-- Complete the quest
function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "FIFTY LOBSTERS! You absolute legend! Come here, let me... no, I won't hug you. But thank you."
    })

    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "I'm ready to take on the swamp again! With a belly full of lobster, nothing can stop me. You've been a true friend."
    })

    ctx:complete_quest()
end

-- Post-completion dialogue
function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Swamp Hermit",
        text = "Still full of lobster! Best meal I've had in years. Thanks to you, I'm back in fighting shape."
    })
end
