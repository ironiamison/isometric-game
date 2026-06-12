CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_login TEXT
);

CREATE TABLE IF NOT EXISTS characters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    gender TEXT NOT NULL DEFAULT 'male',
    skin TEXT NOT NULL DEFAULT 'tan',
    hair_style INTEGER,
    hair_color INTEGER,
    x REAL NOT NULL DEFAULT -30.0,
    y REAL NOT NULL DEFAULT 19.0,
    z INTEGER NOT NULL DEFAULT 0,
    hp INTEGER NOT NULL DEFAULT 10,
    max_hp INTEGER NOT NULL DEFAULT 10,
    level INTEGER NOT NULL DEFAULT 3,
    prayer_points INTEGER,
    mp INTEGER,
    gold INTEGER NOT NULL DEFAULT 0,
    equipped_head TEXT,
    equipped_body TEXT,
    equipped_weapon TEXT,
    equipped_back TEXT,
    equipped_feet TEXT,
    equipped_ring TEXT,
    equipped_gloves TEXT,
    equipped_necklace TEXT,
    equipped_belt TEXT,
    inventory_json TEXT NOT NULL DEFAULT '[]',
    skills_json TEXT,
    played_time INTEGER NOT NULL DEFAULT 0,
    monster_kills INTEGER NOT NULL DEFAULT 0,
    is_admin INTEGER NOT NULL DEFAULT 0 CHECK (is_admin IN (0, 1)),
    current_map TEXT,
    sitting_at_x INTEGER,
    sitting_at_y INTEGER,
    entrance_x REAL,
    entrance_y REAL,
    bank_json TEXT NOT NULL DEFAULT '[]',
    bank_gold INTEGER NOT NULL DEFAULT 0,
    bank_max_slots INTEGER NOT NULL DEFAULT 50 CHECK (bank_max_slots > 0),
    bank_stacks_consolidated INTEGER NOT NULL DEFAULT 1 CHECK (bank_stacks_consolidated IN (0, 1)),
    combat_style_prefs TEXT NOT NULL DEFAULT '{}',
    active_title TEXT,
    commission_marks INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_characters_account_id ON characters(account_id);

CREATE TABLE IF NOT EXISTS character_quests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    quest_id TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'active',
    objectives_json TEXT NOT NULL DEFAULT '{}',
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    UNIQUE(character_id, quest_id),
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS character_flags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_id INTEGER NOT NULL,
    flag_name TEXT NOT NULL,
    flag_value TEXT,
    UNIQUE(character_id, flag_name),
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS character_quest_availability (
    character_id INTEGER NOT NULL,
    quest_id TEXT NOT NULL,
    unlocked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(character_id, quest_id),
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS arena_stats (
    character_id INTEGER PRIMARY KEY,
    total_wins INTEGER NOT NULL DEFAULT 0,
    total_matches INTEGER NOT NULL DEFAULT 0,
    total_kills INTEGER NOT NULL DEFAULT 0,
    total_deaths INTEGER NOT NULL DEFAULT 0,
    current_streak INTEGER NOT NULL DEFAULT 0,
    best_streak INTEGER NOT NULL DEFAULT 0,
    total_gold_won INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS farming_patches (
    patch_id TEXT NOT NULL,
    player_id TEXT NOT NULL,
    crop_id TEXT NOT NULL,
    planted_at INTEGER NOT NULL,
    PRIMARY KEY (patch_id, player_id)
);

CREATE TABLE IF NOT EXISTS farming_plot_unlocks (
    player_id TEXT NOT NULL,
    plot_id INTEGER NOT NULL,
    unlocked_at INTEGER NOT NULL,
    PRIMARY KEY (player_id, plot_id)
);

CREATE TABLE IF NOT EXISTS farming_contracts (
    player_id TEXT PRIMARY KEY,
    difficulty TEXT NOT NULL,
    crop_id TEXT NOT NULL,
    amount_required INTEGER NOT NULL CHECK (amount_required > 0),
    amount_harvested INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS resource_contracts (
    player_id TEXT PRIMARY KEY,
    contract_kind TEXT NOT NULL,
    difficulty TEXT NOT NULL,
    target_item_id TEXT NOT NULL,
    target_name TEXT NOT NULL,
    amount_required INTEGER NOT NULL CHECK (amount_required > 0),
    amount_completed INTEGER NOT NULL DEFAULT 0,
    giver_npc_id TEXT NOT NULL,
    giver_name TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS resource_contract_stats (
    player_id TEXT PRIMARY KEY,
    contracts_completed INTEGER NOT NULL DEFAULT 0,
    total_gold_earned INTEGER NOT NULL DEFAULT 0,
    total_xp_earned INTEGER NOT NULL DEFAULT 0,
    daily_completed INTEGER NOT NULL DEFAULT 0,
    daily_date TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS friendships (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    requester_id INTEGER NOT NULL,
    recipient_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(requester_id, recipient_id),
    CHECK (requester_id <> recipient_id),
    FOREIGN KEY (requester_id) REFERENCES characters(id) ON DELETE CASCADE,
    FOREIGN KEY (recipient_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_friendships_requester ON friendships(requester_id);
CREATE INDEX IF NOT EXISTS idx_friendships_recipient ON friendships(recipient_id);

CREATE TABLE IF NOT EXISTS discovered_recipes (
    character_id INTEGER NOT NULL,
    recipe_id TEXT NOT NULL,
    discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (character_id, recipe_id),
    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS character_unlocked_spells (
    character_id INTEGER NOT NULL,
    spell_id TEXT NOT NULL,
    unlocked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (character_id, spell_id),
    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS character_slayer (
    character_id INTEGER PRIMARY KEY,
    slayer_state_json TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS chests (
    chest_key TEXT PRIMARY KEY,
    slots_json TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS koth_pending_rewards (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_koth_rewards_player ON koth_pending_rewards(player_id);

CREATE TABLE IF NOT EXISTS boss_pending_rewards (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_boss_rewards_player ON boss_pending_rewards(player_id);

CREATE TABLE IF NOT EXISTS bans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    ip_address TEXT,
    banned_by TEXT NOT NULL,
    reason TEXT,
    banned_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_bans_account_expiry ON bans(account_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_bans_ip_expiry ON bans(ip_address, expires_at);

CREATE TABLE IF NOT EXISTS collection_log (
    character_id INTEGER NOT NULL,
    item_id TEXT NOT NULL,
    source TEXT NOT NULL,
    source_detail TEXT,
    obtained_at TEXT NOT NULL,
    PRIMARY KEY (character_id, item_id, source),
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS player_titles (
    character_id INTEGER NOT NULL,
    title_id TEXT NOT NULL,
    unlocked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (character_id, title_id),
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS crafting_orders_available (
    character_id INTEGER NOT NULL,
    order_id TEXT NOT NULL,
    generated_date TEXT NOT NULL,
    PRIMARY KEY (character_id, order_id),
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS crafting_orders_generation (
    character_id INTEGER PRIMARY KEY,
    generated_date TEXT NOT NULL,
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS crafting_orders_active (
    character_id INTEGER PRIMARY KEY,
    order_id TEXT NOT NULL,
    accepted_at INTEGER NOT NULL,
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS crafting_order_stats (
    character_id INTEGER PRIMARY KEY,
    orders_completed INTEGER NOT NULL DEFAULT 0,
    masterwork_completed INTEGER NOT NULL DEFAULT 0,
    total_marks_earned INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(character_id) REFERENCES characters(id) ON DELETE CASCADE
);
