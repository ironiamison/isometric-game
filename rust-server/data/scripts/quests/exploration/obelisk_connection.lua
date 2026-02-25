-- Obelisk Connection Quest Script
-- Given by Researcher Orin near the southern obelisk

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
        speaker = "Researcher Orin",
        text = "Ah, a traveler! You see this obelisk? It's no ordinary stone. I've spent weeks studying its resonance - it pulses with ancient magic. I believe there's another one far to the north, and they were once connected. Something has severed the link.\n\nI can't leave my research here, but... would you be willing to find the other obelisk and try to restore the connection?",
        choices = {
            { id = "accept", text = "I'll find it." },
            { id = "decline", text = "Not right now." },
            { id = "ask_more", text = "What kind of magic?" }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "Wonderful! Head north - far north. The second obelisk should be deep in the wilderness. When you find it, try touching it. The stones seem to respond to people. Good luck, traveler!"
        })
    elseif choice == "ask_more" then
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "Teleportation magic, I believe. In the old texts, they called them waystones - paired monuments that could transport someone from one to the other in an instant. Imagine the possibilities!"
        })
        return show_offer_dialogue(ctx)
    else
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "No rush, friend. The stones have been here for centuries. They'll wait a little longer."
        })
    end
end

function show_progress_dialogue(ctx)
    local reach = ctx:get_objective_progress("reach_north_obelisk")
    local dig = ctx:get_objective_progress("dig_at_blockage")
    local kill = ctx:get_objective_progress("kill_hedgehog")

    if reach.current < reach.target then
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "Still here? The northern obelisk is a long journey from here. Head north and keep your eyes open - you'll know it when you see it."
        })
    elseif dig.current < dig.target then
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "You found the obelisk? Excellent! I've heard there's some disturbed ground nearby - try using a shovel to dig around the area. Something may be lurking beneath the surface that's disrupting the connection."
        })
    elseif kill.current < kill.target then
        ctx:show_dialogue({
            speaker = "Researcher Orin",
            text = "You found it? But there's something blocking the connection? Hmm... the old texts mention that sometimes creatures nest near sources of magical energy. You may need to clear whatever is disrupting it."
        })
    end
end

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Researcher Orin",
        text = "You did it! I can feel the resonance humming between the stones - the connection is alive again! Both obelisks should now respond to your touch. Step up to either one and you can travel to the other in an instant. Remarkable work, truly remarkable!"
    })
    ctx:complete_quest()
    ctx:unlock_waystone("south_obelisk")
    ctx:unlock_waystone("north_obelisk")
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Researcher Orin",
        text = "The resonance is strong today. The waystones are working beautifully. Thank you again, traveler."
    })
end
