-- City of Sparks - Quest 1 of The Awakening
-- Guard Captain Aldric sends the player to investigate disturbance sites

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
            speaker = "Guard Captain Aldric",
            text = "Archmage Yenara speaks highly of you. If you're heading out there again, be careful."
        })
    end
end

function show_offer_dialogue(ctx)
    local choice = ctx:show_dialogue({
        speaker = "Guard Captain Aldric",
        text = "Adventurer! We've got a crisis. Magic is going haywire all over New Aeven - enchanted lanterns exploding, objects coming to life. I need someone to investigate three disturbance sites.",
        choices = {
            { id = "accept", text = "I'll investigate." },
            { id = "ask", text = "What's causing this?" },
            { id = "decline", text = "Not right now." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Guard Captain Aldric",
            text = "Check the market district, the mage college courtyard, and the city gate. Report anything you find to Archmage Yenara at the college."
        })
    elseif choice == "ask" then
        ctx:show_dialogue({
            speaker = "Guard Captain Aldric",
            text = "If I knew that, I wouldn't need help. The Archmage thinks it's something underground. All I know is my guards are getting attacked by enchanted brooms."
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Guard Captain Aldric",
            text = "I understand, but people are getting hurt. Come back if you change your mind."
        })
    end
end

function show_progress_dialogue(ctx)
    local market = ctx:get_objective_progress("investigate_market")
    local college = ctx:get_objective_progress("investigate_college")
    local gate = ctx:get_objective_progress("investigate_gate")

    local done = 0
    if market.current >= market.target then done = done + 1 end
    if college.current >= college.target then done = done + 1 end
    if gate.current >= gate.target then done = done + 1 end

    ctx:show_dialogue({
        speaker = "Guard Captain Aldric",
        text = string.format("You've investigated %d of 3 sites. Archmage Yenara is waiting for your full report.", done)
    })
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Guard Captain Aldric",
        text = "You've checked all three? Good. Get to Archmage Yenara at the college - she'll want to hear everything."
    })
    ctx:complete_quest()
end

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "report_to_yenara" then
        ctx:show_notification("Report delivered to Archmage Yenara.")
    end
end
