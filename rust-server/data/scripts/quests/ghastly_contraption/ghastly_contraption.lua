-- A Ghastly Contraption
-- Player helps Professor Oddwick clear a haunted house and build the Leather Attractor.
--
-- NPC routing: Both Oddwick and Barnaby list this quest in available_quests.
-- The script checks objective progress to determine which NPC the player is
-- talking to and routes to the appropriate dialogue handler.

function on_interact(ctx)
    local quest_state = ctx:get_quest_state()

    if quest_state == "not_started" then
        show_oddwick_offer(ctx)
        return
    end

    if quest_state == "completed" then
        show_post_complete(ctx)
        return
    end

    if quest_state == "ready_to_complete" then
        show_oddwick_build(ctx)
        return
    end

    -- in_progress: route based on objective state
    route_in_progress(ctx)
end

-- ============================================================================
-- NPC Routing (in_progress state)
-- ============================================================================

function route_in_progress(ctx)
    local npc = ctx:get_interacting_npc()
    local tinderbox = ctx:get_objective_progress("find_tinderbox")
    local gate = ctx:get_objective_progress("open_first_gate")
    local barnaby = ctx:get_objective_progress("talk_barnaby")

    -- If tinderbox not found yet, route based on which NPC
    if tinderbox.current < tinderbox.target then
        if npc == "haunted_bookshelf" then
            show_bookshelf_search(ctx)
        else
            show_oddwick_hint_tinderbox(ctx)
        end
        return
    end

    -- If gate not opened yet, player has tinderbox — hint to use tinderbox on candles
    if gate.current < gate.target then
        if npc == "haunted_candles" then
            ctx:show_dialogue({
                speaker = "Narrator",
                text = "An ornate candle stand. Use a tinderbox to light it."
            })
        elseif npc == "prof_oddwick" then
            show_oddwick_hint_candles(ctx)
        else
            ctx:show_dialogue({
                speaker = "Narrator",
                text = "You need to find a way past the gate."
            })
        end
        return
    end

    -- If Barnaby not convinced yet, this is the Barnaby interaction
    if barnaby.current < barnaby.target then
        if npc == "barnaby_ghost" then
            show_barnaby_interrogation(ctx)
        else
            show_oddwick_hint_barnaby(ctx)
        end
        return
    end

    -- Otherwise player is mid-quest talking to Oddwick (or Barnaby post-key)
    if npc == "barnaby_ghost" then
        show_barnaby_post_key(ctx)
    else
        show_oddwick_waiting(ctx)
    end
end

-- ============================================================================
-- Step 1: Oddwick Offer
-- ============================================================================

function show_oddwick_offer(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Oh! A real person! You have no idea how glad I am to see you."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I bought this house at auction. A steal! ...Literally. The previous owner's ghost stole all the furniture."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I've been working on a device to neutralize the spectral energy, but I need components from the basement. Problem is, it's locked behind gates — and the basement is... well, haunted."
    })

    local choice = ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I need someone brave — or foolish — to help me clear this place out. What do you say?",
        choices = {
            { id = "accept", text = "I'll help you out." },
            { id = "decline", text = "This sounds like your problem." }
        }
    })

    if choice == "accept" then
        ctx:accept_quest()
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "Wonderful! The deeper rooms are locked behind gates — some kind of old candle mechanism. Very dramatic, very impractical."
        })
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "But first, we'll need a tinderbox to light anything. I'm sure there's one around here somewhere — try searching the bookshelves."
        })
    else
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "Fair enough. Can't blame you. But if you change your mind, I'll be here. Not like I can leave — the ghost took my carriage wheels too."
        })
    end
end

-- ============================================================================
-- Bookshelf Search (tinderbox discovery)
-- ============================================================================

function show_bookshelf_search(ctx)
    ctx:show_dialogue({
        speaker = "Narrator",
        text = "You rummage through the dusty bookshelf. Old tomes, cobwebs, a suspicious amount of cat hair..."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "Your hand closes around a small metal box buried behind a stack of mouldy encyclopedias."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "A tinderbox! This should be able to light those candles."
    })

    -- Grant item AFTER the last dialogue so it only fires once
    -- (non-dialogue calls execute on every replay, so they must come
    -- after all show_dialogue calls in the function)
    ctx:give_item("tinderbox", 1)
    ctx:show_notification("Found: Tinderbox")
end

-- ============================================================================
-- Oddwick Hints (tinderbox not yet found)
-- ============================================================================

function show_oddwick_hint_tinderbox(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "We need a tinderbox before we can do anything with those candles. Have you tried searching the bookshelves? The previous owner kept all sorts of things stuffed between the books."
    })
end

-- ============================================================================
-- Oddwick Hints (candle puzzle not yet solved)
-- ============================================================================

function show_oddwick_hint_candles(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "You found the tinderbox! Excellent! Now — the candles by the gate need to be lit in a specific order."
    })
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I think the order was... skull candle first, then the tall one, then... red? No wait, I think the tall one was first. Or was skull second? Blast, I can't remember exactly."
    })
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Just try different combinations near the gate. You'll figure it out! ...Probably."
    })
end

-- ============================================================================
-- Oddwick Hints (Barnaby not yet found)
-- ============================================================================

function show_oddwick_hint_barnaby(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "You got past the gate! Now you need to find a key to the basement. I've heard someone — or someTHING — rattling around deeper in the house. Maybe they know where it is."
    })
end

-- ============================================================================
-- Barnaby post-key (already gave the key)
-- ============================================================================

function show_barnaby_post_key(ctx)
    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Still here? Don't forget — the basement is that way. Be careful! I hear angry noises down there. ...Or it's the pipes. Hard to tell."
    })
end

-- ============================================================================
-- Use Item on Entity — Candle Puzzle
-- ============================================================================

local CANDLE_INFO = {
    candle_1 = { name = "skull candle", flame = "an eerie green" },
    candle_2 = { name = "tall taper", flame = "a pale blue" },
    candle_3 = { name = "red candle", flame = "a warm orange" },
    candle_4 = { name = "small stubby candle", flame = "a sputtering yellow" },
}

local CANDLE_ORDER = { "candle_1", "candle_2", "candle_3", "candle_4" }

function on_use_item(ctx, item_id, entity_type, npc_id)
    if item_id ~= "tinderbox" or entity_type ~= "haunted_candles" then
        return false
    end

    local gate = ctx:get_objective_progress("open_first_gate")
    if gate.current >= gate.target then
        ctx:show_notification("The candles are already lit. The gate is open.")
        return true
    end

    local tinderbox = ctx:get_objective_progress("find_tinderbox")
    if tinderbox.current < tinderbox.target then
        ctx:show_notification("You need to find a tinderbox first.")
        return true
    end

    -- Get current lit candles from flag
    local lit_str = ctx:get_flag("candles_lit")
    if lit_str == nil then lit_str = "" end
    local lit = {}
    if lit_str ~= "" then
        for id in string.gmatch(lit_str, "([^,]+)") do
            table.insert(lit, id)
        end
    end

    -- Check if already lit
    for _, id in ipairs(lit) do
        if id == npc_id then
            local info = CANDLE_INFO[npc_id]
            if info then
                ctx:show_notification("The " .. info.name .. " is already lit.")
            end
            return true
        end
    end

    -- Check if correct next candle
    local next_index = #lit + 1
    local expected = CANDLE_ORDER[next_index]

    if npc_id ~= expected then
        ctx:set_flag("candles_lit", "")
        ctx:show_notification("A cold wind howls through the room. All the candles snuff out at once. Somewhere, a ghost laughs.")
        return true
    end

    -- Correct! Light it
    table.insert(lit, npc_id)
    ctx:set_flag("candles_lit", table.concat(lit, ","))

    local info = CANDLE_INFO[npc_id]
    if info then
        ctx:show_notification("The " .. info.name .. " flickers to life with " .. info.flame .. " flame.")
    end

    -- All 4 lit?
    if #lit == #CANDLE_ORDER then
        ctx:show_notification("All four candles burn in unison. The gate groans... and slowly creaks open!")
        ctx:complete_objective("open_first_gate")
        ctx:set_flag("candles_lit", "")
    end

    return true
end

-- ============================================================================
-- Step 3: Barnaby's "Prove You're Alive" Interrogation
-- ============================================================================

function show_barnaby_interrogation(ctx)
    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Oh! A visitor! How exciting! ...Wait."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Are you alive? Or are you one of THEM? I've had ghosts try to trick me before. Well, I think they were ghosts. Hard to tell these days."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "I'm going to need you to prove you're alive. I have a very rigorous three-question test. Ready?"
    })

    -- Question 1: Do you breathe?
    local q1 = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Question one: Do you breathe?",
        choices = {
            { id = "obviously", text = "Yes, obviously." },
            { id = "watch", text = "Watch me." },
            { id = "do_you", text = "Do YOU breathe?" }
        }
    })

    if q1 == "obviously" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Hmm... that's EXACTLY what a ghost pretending to breathe would say. Suspicious."
        })
    elseif q1 == "watch" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Oh! Your chest moves! ...Unless that's a trick. But I'll give you the benefit of the doubt."
        })
    elseif q1 == "do_you" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Of course I do! I'm perfectly alive! ...Aren't I? Anyway, this is about YOU."
        })
    end

    -- Question 2: What's your favorite food?
    local q2 = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Question two: What's your favorite food?",
        choices = {
            { id = "none", text = "I don't eat." },
            { id = "bread", text = "Bread and stew." },
            { id = "ecto", text = "Ectoplasm." }
        }
    })

    if q2 == "none" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "AHA! Ghost confirmed! ...Wait, you could just be on a diet. Hmm. Proceed."
        })
    elseif q2 == "bread" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Ooh, bread and stew! That does sound like a living person thing. I miss stew. ...Do I miss stew? I can't remember."
        })
    elseif q2 == "ecto" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "See, I KNEW— wait, really? That's disgusting even for a ghost. ...Are you feeling alright?"
        })
    end

    -- Question 3: Can you walk through walls?
    local q3 = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Final question: Can you walk through walls?",
        choices = {
            { id = "yes", text = "Yes." },
            { id = "door", text = "No, I used the door." },
            { id = "can_you", text = "Can YOU?" }
        }
    })

    if q3 == "yes" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Ghost! I knew it! ...Actually, wait, you haven't floated through anything since you got here. I'll let it slide."
        })
    elseif q3 == "door" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "The DOOR? Nobody uses doors anymore! That's so old-fashioned. ...Maybe you ARE alive."
        })
    elseif q3 == "can_you" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Can I? Watch this!"
        })
        ctx:show_dialogue({
            speaker = "Narrator",
            text = "Barnaby floats through a wall and back, looking very pleased with himself."
        })
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "See? Easy! ...Wait, can you not do that? Oh dear."
        })
    end

    -- All questions done — Barnaby is convinced (regardless of answers)
    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Alright, alright, I believe you. You're alive. How exciting! I haven't talked to a living person in... actually, how long have I been here?"
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "So what brings you to my humble haunted house? ...Well, I suppose it's technically the professor's now."
    })

    local key_choice = ctx:show_dialogue({
        speaker = "Player",
        text = "I need to get into the basement. Do you have a key?",
        choices = {
            { id = "ask", text = "Do you have a key?" }
        }
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "A key? You mean this old thing?"
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "Barnaby pulls a tarnished iron key from... somewhere. Best not to think about where a ghost keeps things."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "I found it years ago. It's my lucky charm! ...Has it been lucky? I can't remember. I can't remember a lot of things, actually."
    })

    local take_choice = ctx:show_dialogue({
        speaker = "Barnaby",
        text = "You want it? I suppose I don't really NEED luck. I'm already dead. ...Wait. What did I just say?",
        choices = {
            { id = "take", text = "Thanks, Barnaby." },
            { id = "gentle", text = "You're a good ghost, Barnaby." }
        }
    })

    if take_choice == "gentle" then
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "A good... ghost? I'm a ghost? ...Huh. That actually explains a LOT."
        })
    else
        ctx:show_dialogue({
            speaker = "Barnaby",
            text = "Don't mention it! And be careful down there. I hear things. Angry things. ...Could also be the plumbing."
        })
    end

    -- Grant item AFTER the last dialogue to avoid double-granting during replay
    ctx:give_item("basement_key", 1)
    ctx:show_notification("Received: Basement Key")
end

-- Note on give_item placement: non-dialogue calls (give_item, show_notification)
-- execute on EVERY script replay. Dialogues get skipped during replay, but
-- give_item does not. Always place give_item after the last show_dialogue in a
-- function to ensure it only fires once (when the script runs to completion).

-- ============================================================================
-- Oddwick Waiting (mid-quest, after Barnaby)
-- ============================================================================

function show_oddwick_waiting(ctx)
    local poltergeist = ctx:get_objective_progress("defeat_poltergeist")
    local ectoplasm = ctx:get_objective_progress("collect_ectoplasm")
    local coil = ctx:get_objective_progress("collect_coil")

    if poltergeist.current < poltergeist.target then
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "You have the basement key? Excellent! Get down there and deal with whatever's causing all this ruckus. I'll be here. Preparing. Definitely not hiding."
        })
    elseif ectoplasm.current < ectoplasm.target or coil.current < coil.target then
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "You defeated it?! Did it drop anything? I need spectral components — ectoplasm, a coil, anything that glows ominously!"
        })
    else
        ctx:show_dialogue({
            speaker = "Professor Oddwick",
            text = "You have the components? Quick, hand them over! I've been preparing the assembly rig!"
        })
    end
end

-- ============================================================================
-- Step 5: Oddwick Build Sequence (quest completion)
-- ============================================================================

function show_oddwick_build(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "You got them! Haunted ectoplasm AND a spectral coil! Do you know how rare these are?"
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Well, I don't either, but they FEEL rare. Now hold still while I calibrate the ectoplasmic resonance matrix..."
    })

    -- First attempt: failure
    ctx:show_dialogue({
        speaker = "Narrator",
        text = "Oddwick connects the coil to a leather harness, pours the ectoplasm into a glass chamber, and starts cranking a handle. Sparks fly. The device rattles violently."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "BANG! A small explosion rocks the table. Smoke fills the room."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "...That wasn't supposed to happen."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Is it supposed to be on fire?"
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "No, Barnaby. Thank you for your observation."
    })

    -- Second attempt: success
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Right. Slight adjustment... reverse the polarity... carry the two..."
    })

    ctx:show_dialogue({
        speaker = "Narrator",
        text = "A soft hum fills the air. The device glows with a gentle, steady light. It's working."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "IT WORKS! Behold — the Leather Attractor! It uses spectral energy to magnetically recall projectiles. Arrows, bolts — they'll come right back to you!"
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Well, about 60% of the time. The other 40%... we don't talk about the other 40%."
    })

    ctx:show_dialogue({
        speaker = "Barnaby",
        text = "Can it recall my memories? I can't remember where I left my body."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "...No, Barnaby."
    })

    ctx:complete_quest()

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Take it! You've earned it. And thank you — the house already feels less... murdery."
    })
end

-- ============================================================================
-- Post-Quest: Completed dialogue + Upgrade offer
-- ============================================================================

function show_post_complete(ctx)
    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Ah, my favorite ghost-hunter! The house has been much quieter since you dealt with that poltergeist."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "I've been tinkering with the attractor design. I think I can enhance the recovery field — push it up to 72% — but I'll need rare materials."
    })

    ctx:show_dialogue({
        speaker = "Professor Oddwick",
        text = "Bring me your Leather Attractor and 6 Ancient Fragments, and I'll build you the Improved Attractor. You'll also need Ranged level 50 to handle the increased spectral feedback."
    })
end

-- ============================================================================
-- Objective Progress Notifications
-- ============================================================================

function on_objective_progress(ctx, objective_id, new_count)
    if objective_id == "defeat_poltergeist" and new_count == 1 then
        ctx:show_notification("The poltergeist has been vanquished! Collect the remains and return to Oddwick.")
    end
end
