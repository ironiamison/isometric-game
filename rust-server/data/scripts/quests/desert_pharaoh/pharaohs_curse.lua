-- The Pharaoh's Curse - Quest 1 of the Desert Pharaoh chain
-- Player investigates an ancient cursed pharaoh through NPC conversations,
-- solves a riddle, and fights a boss beneath the pyramid.

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        return show_offer_dialogue(ctx)
    elseif quest_state == "in_progress" then
        return show_progress_dialogue(ctx)
    elseif quest_state == "ready_to_complete" then
        return complete_quest(ctx)
    elseif quest_state == "completed" then
        show_completed_dialogue(ctx)
    end
end

-- ============================================================================
-- Quest Offer (Desert Merchant)
-- ============================================================================

function show_offer_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "Ah, a traveler! Business has been terrible lately. Nobody wants to come near the pyramid anymore."
    })

    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "I hear chanting from beneath the sand at night. The locals whisper about an ancient pharaoh named Kha'reth who was sealed away long ago."
    })

    local choice = ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "If you're brave enough to investigate, you should speak to the Nomad Elder at the oasis. He knows the old stories better than anyone.",
        choices = {
            { id = "accept", text = "I'll look into it." },
            { id = "decline", text = "That sounds too dangerous." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Desert Merchant",
            text = "Good luck out there. The elder's camp is at the oasis. And remember the name - Kha'reth. You'll need it."
        })
    else
        ctx:show_dialogue({
            speaker = "Desert Merchant",
            text = "Can't blame you. But if you change your mind, I'll be here."
        })
    end
end

-- ============================================================================
-- Progress Dialogue (dispatches based on current objective)
-- ============================================================================

function show_progress_dialogue(ctx)
    local merchant = ctx:get_objective_progress("talk_merchant")
    local elder = ctx:get_objective_progress("talk_elder")
    local researcher = ctx:get_objective_progress("talk_researcher")
    local hermit = ctx:get_objective_progress("talk_hermit")

    -- Nomad Elder dialogue
    if merchant.current >= merchant.target and elder.current < elder.target then
        show_elder_dialogue(ctx)
        return
    end

    -- Tomb Researcher dialogue
    if elder.current >= elder.target and researcher.current < researcher.target then
        show_researcher_dialogue(ctx)
        return
    end

    -- Desert Hermit dialogue (riddle)
    if researcher.current >= researcher.target and hermit.current < hermit.target then
        show_hermit_dialogue(ctx)
        return
    end

    -- Default progress reminder
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "Still investigating? Keep talking to the people who know this land."
    })
end

-- ============================================================================
-- Nomad Elder — tells the legend, clue: "three stars of the southern sky"
-- ============================================================================

function show_elder_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "Sit, young one. You ask about the pyramid? Then I will tell you of Kha'reth."
    })

    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "He was a pharaoh who enslaved his people to build a tomb that would grant him eternal life. But the magic he sought was dark and corrupting."
    })

    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "When his priests saw what he had become, they sealed him inside. The binding was powerful - tied to the three stars of the southern sky."
    })

    ctx:show_dialogue({
        speaker = "Nomad Elder",
        text = "But seals weaken with time. If the chanting has returned... the binding may be failing. Seek the researcher near the pyramid - she has been studying the inscriptions."
    })
end

-- ============================================================================
-- Tomb Researcher — clue: "blood of the scorpion"
-- ============================================================================

function show_researcher_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "You've spoken to the elder? Good. Then you know what we're dealing with."
    })

    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "I've been translating the inscriptions on these walls for months. They describe a ritual that Kha'reth performed - one that required the blood of the scorpion."
    })

    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "Others have gone inside to investigate. None of them came back. There's a locked door deep within the pyramid that no one has been able to open."
    })

    ctx:show_dialogue({
        speaker = "Tomb Researcher",
        text = "If you're truly going after this... there's a hermit who lives in a hidden house out in the desert. He's spent decades studying Kha'reth. He may know how to get past that door."
    })
end

-- ============================================================================
-- Desert Hermit — the riddle puzzle
-- ============================================================================

function show_hermit_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "So... someone finally comes seeking the truth about Kha'reth. I've waited a long time for this."
    })

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "I have the book that holds the key - literally. But I cannot give it to just anyone. You must prove you understand the story."
    })

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "The book asks three questions. Answer them from what you've learned, and the key is yours."
    })

    -- Riddle Question 1: Name of the cursed one
    local q1 = ctx:show_dialogue({
        speaker = "Ancient Book",
        text = "Speak the name of the cursed one.",
        choices = {
            { id = "khareth", text = "Kha'reth" },
            { id = "wrong1a", text = "Osirath" },
            { id = "wrong1b", text = "Amenhotep" }
        }
    })

    if q1 ~= "khareth" then
        show_riddle_failure(ctx)
        return
    end

    -- Riddle Question 2: The binding above
    local q2 = ctx:show_dialogue({
        speaker = "Ancient Book",
        text = "Name the binding above.",
        choices = {
            { id = "wrong2a", text = "The light of the sun" },
            { id = "three_stars", text = "Three stars of the southern sky" },
            { id = "wrong2b", text = "The desert winds" }
        }
    })

    if q2 ~= "three_stars" then
        show_riddle_failure(ctx)
        return
    end

    -- Riddle Question 3: The price below
    local q3 = ctx:show_dialogue({
        speaker = "Ancient Book",
        text = "Name the price below.",
        choices = {
            { id = "wrong3a", text = "Tears of the fallen" },
            { id = "wrong3b", text = "Gold of the kingdom" },
            { id = "scorpion_blood", text = "Blood of the scorpion" }
        }
    })

    if q3 ~= "scorpion_blood" then
        show_riddle_failure(ctx)
        return
    end

    -- All three correct!
    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "The book trembles... the pages glow with ancient light..."
    })

    ctx:give_item("pharaohs_key", 1)

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "The Pharaoh's Key. It will open the sealed door within the pyramid. But be warned - what lies beyond has had millennia to grow in power."
    })

    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "Go to the pyramid. Find the locked door deep inside. And may the stars protect you from what sleeps below."
    })
end

function show_riddle_failure(ctx)
    ctx:show_dialogue({
        speaker = "Desert Hermit",
        text = "That's not right. The book snaps shut. Perhaps you should speak to more people and learn the full story before trying again."
    })
end

-- ============================================================================
-- Quest Complete
-- ============================================================================

function complete_quest(ctx)
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "You... you went down there? And survived?!"
    })

    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "The chanting has stopped. The desert feels lighter somehow. Whatever you did down there... thank you."
    })

    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "But I wonder... was Kha'reth the only thing sealed beneath these sands? The elder spoke of others..."
    })

    ctx:complete_quest()
end

function show_completed_dialogue(ctx)
    ctx:show_dialogue({
        speaker = "Desert Merchant",
        text = "The pyramid is quiet now, thanks to you. But sometimes I still feel something watching from beneath the sand..."
    })
end

-- ============================================================================
-- Objective Progress Notifications
-- ============================================================================

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "defeat_khareth" and new_count == 1 then
        ctx:show_notification("Kha'reth has been defeated! Return to the Desert Merchant.")
    end
end
