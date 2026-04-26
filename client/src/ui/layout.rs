use macroquad::prelude::{Rect, Vec2};

/// Identifier for a clickable UI element
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiElementId {
    // Dialogue
    DialogueChoice(usize),
    DialogueContinue,
    DialogueClose,
    DialogueScrollbar,
    AdventurerTab(usize),
    AdventurerTier(usize),
    AdventureBoardOffer(usize),
    AdventureBoardDifficulty(usize),
    AdventureBoardClaim,
    AdventureBoardAbandon,

    // Crafting
    CraftingCategoryTab(usize),
    CraftingRecipeItem(usize),
    CraftingScrollArea,
    CraftingScrollbar,
    CraftingButton,
    CraftingCancelButton,
    MainTab(usize), // 0=Recipes, 1=Shop

    // Inventory & Quick Slots
    InventorySlot(usize),
    QuickSlot(usize),

    // Equipment Slots
    EquipmentSlot(String), // e.g., "body"

    // Quest Log
    QuestLogEntry(usize),
    QuestLogScrollArea,
    QuestLogScrollbar,
    QuestDetailBack,

    // Collection Log
    QuestsTab,
    CollectionLogTab,
    CollectionLogCategory(usize),
    CollectionLogSubcategory(usize),
    CollectionLogBack,
    CollectionLogScrollArea,
    CollectionLogScrollbar,

    // Context Menu
    ContextMenuOption(usize),

    // Escape Menu
    EscapeMenuZoom05x,
    EscapeMenuZoom1x,
    EscapeMenuZoom2x,
    EscapeMenuMusicSlider,
    EscapeMenuSfxSlider,
    EscapeMenuMuteToggle,
    EscapeMenuUiScaleSlider,
    EscapeMenuShiftDropToggle,
    EscapeMenuChatLogToggle,
    EscapeMenuChatBgToggle,
    EscapeMenuTapPathfindToggle,
    EscapeMenuJoystickToggle,
    EscapeMenuControlSchemeToggle,
    EscapeMenuGraphicsToggle,
    EscapeMenuDisconnect,

    // World Items
    GroundItem(String), // item instance ID

    // Shop/Crafting close button
    ShopCraftingCloseButton,

    // Shop
    ShopBuyItem(usize),
    ShopSellItem(usize),
    ShopBuyScrollArea,
    ShopSellScrollArea,
    ShopBuyScrollbar,
    ShopSellScrollbar,
    ShopBuyQuantityMinus,
    ShopBuyQuantityPlus,
    ShopBuyConfirmButton,
    ShopSellQuantityMinus,
    ShopSellQuantityPlus,
    ShopSellQuantityMax,
    ShopSellConfirmButton,

    // Menu Buttons
    MenuButtonInventory,
    MenuButtonCharacter,
    MenuButtonSocial,
    MenuButtonSkills,
    MenuButtonPrayer,
    MenuButtonQuest,
    MenuButtonSettings,
    MenuButtonToggle,

    // Skills Panel
    SkillSlot(usize),

    // Prayer Panel
    PrayerSlot(usize),

    // Spell Panel (shares with prayer panel)
    SpellSlot(usize),
    PrayerSpellTab(usize), // 0 = Prayers tab, 1 = Spells tab

    // Unified Hotkey Bar
    HotkeyPresetUp,
    HotkeyPresetDown,
    HotkeySettingsCog,
    HotkeySettingsPresetTab(usize),
    HotkeySettingsSlot(usize),
    HotkeySettingsSlotClear(usize),
    // Gold Display (inventory header)
    GoldDisplay,

    // Inventory grid scroll area
    InventoryGridArea,
    InventoryScrollbar,

    // Gold Drop Dialog
    GoldDropConfirm,
    GoldDropCancel,

    // Stall Price Dialog
    StallPriceConfirm,
    StallPriceCancel,

    // Bank Quantity Dialog
    BankQuantityConfirm,
    BankQuantityMax,
    BankQuantityCancel,

    // Chat Panel
    ChatButton,
    ChatTabLocal,
    ChatTabGlobal,
    ChatTabSystem,
    ChatInputField,
    ChatSendButton,
    ChatCloseButton,
    ChatPanelBackground,
    ChatMessageArea,
    ChatLogArea,
    ChatLogScrollbar,
    ChatPanelScrollbar,

    // Social Panel
    SocialTabNearby,
    SocialTabOnline,
    SocialTabFriends,
    SocialPlayerRow(usize),
    SocialFriendRow(usize),
    SocialRequestAccept(usize),
    SocialRequestDecline(usize),
    SocialRemoveFriend(usize),
    SocialAddFriendInput,
    SocialAddFriendButton,
    SocialPanelClose,
    SocialScrollArea,

    // Bank
    BankSlot(usize),
    BankInventorySlot(usize),
    BankCloseButton,
    BankDepositGoldButton,
    BankWithdrawGoldButton,
    BankScrollArea,
    BankInvScrollArea,
    BankScrollbar,
    BankInvScrollbar,
    BankHelpButton,
    BankHelpClose,
    BankDepositAllButton,
    BankSortButton,

    // Altar Panel
    AltarOfferAll(usize),
    AltarPray,
    AltarClose,

    // Prayer/Spell Help
    PrayerHelpButton,
    SpellHelpButton,
    PrayerHelpClose,
    SpellHelpClose,

    // Minimap
    MinimapToggle,
    MinimapPanel,
    MinimapClose,
    MinimapMarker(usize),

    // Furnace
    FurnaceRecipeItem(usize),
    FurnaceSmeltButton,
    FurnaceCancelButton,
    FurnaceCloseButton,
    FurnaceQuantity1,
    FurnaceQuantityX,
    FurnaceQuantityAll,
    FurnaceScrollArea,
    FurnaceScrollbar,
    FurnaceTabSmelting,
    FurnaceTabJewelry,

    // Anvil
    AnvilRecipeCell(usize),
    AnvilSmithButton,
    AnvilCancelButton,
    AnvilCloseButton,
    AnvilQuantity1,
    AnvilQuantityX,
    AnvilQuantityAll,
    AnvilScrollArea,
    AnvilScrollbar,
    AnvilTabMaterials,
    AnvilTabEquipment,

    // Alchemy Station
    AlchemyCloseButton,
    AlchemyTab(usize),
    AlchemyRecipeItem(usize),
    AlchemyScrollArea,
    AlchemyScrollbar,
    AlchemyBrewButton,
    AlchemyCancelButton,
    AlchemyQuantityMinus,
    AlchemyQuantityPlus,
    AlchemyQuantityMax,

    // Workbench
    WorkbenchCloseButton,
    WorkbenchTab(usize),
    WorkbenchRecipeItem(usize),
    WorkbenchScrollArea,
    WorkbenchScrollbar,
    WorkbenchCraftButton,
    WorkbenchCancelButton,
    WorkbenchQuantityMinus,
    WorkbenchQuantityPlus,

    // Fletching Panel
    FletchingTab(usize),
    FletchingRecipeItem(usize),
    FletchingFletchButton,
    FletchingCancelButton,
    FletchingCloseButton,
    FletchingQuantity1,
    FletchingQuantityX,
    FletchingQuantityAll,
    FletchingScrollArea,
    FletchingScrollbar,

    // Chest Panel
    ChestSlot(u8),
    ChestClose,
    ChestScrollArea,

    // Slayer Panel
    SlayerCloseButton,
    SlayerGetTaskButton,
    SlayerCancelTaskButton,
    SlayerRewardTab(usize),
    SlayerBuyReward(usize),
    SlayerRemoveBlock(usize),
    SlayerBlockMonsterSelect(usize),
    SlayerScrollArea,
    SlayerBlockScrollArea,
    SlayerBlockScrollbar,

    // Trade Panel
    TradeOfferSlot(usize),
    TradePartnerSlot(usize),
    TradeGoldInput,
    TradeAcceptButton,
    TradeCancelButton,
    TradeRequestAccept,
    TradeRequestDecline,

    // Character panel
    CharacterOpenShopButton,
    CombatStyleButton(usize), // 0=Accurate, 1=Aggressive, 2=Defensive, 3=Controlled
    AutoRetaliateToggle,

    // Stall Setup Panel (owner)
    StallSetupSlot(usize),
    StallSetupRemove(usize),
    StallSetupOpenButton,
    StallSetupCloseButton,
    StallSetupNameInput,

    // Stall Browse Panel (buyer)
    StallBrowseItem(usize),
    StallBrowseBuyButton,
    StallBrowseCloseButton,
    StallBrowseQuantityMinus,
    StallBrowseQuantityPlus,

    // KOTH (King of the Hill)
    KothContinueButton,
    KothLeaveButton,
    KothGameOverDismiss,
}

/// A single interactive UI element with its bounds
pub struct UiElement {
    pub id: UiElementId,
    pub bounds: Rect,
}

/// Layout for all interactive elements in the current frame
#[derive(Default)]
pub struct UiLayout {
    pub elements: Vec<UiElement>,
    /// Max scroll values stored by renderers for input handler to clamp against
    scroll_limits: Vec<(UiElementId, f32)>,
}

impl UiLayout {
    pub fn new() -> Self {
        Self {
            elements: Vec::with_capacity(32),
            scroll_limits: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.elements.clear();
        self.scroll_limits.clear();
    }

    pub fn add(&mut self, id: UiElementId, bounds: Rect) {
        self.elements.push(UiElement { id, bounds });
    }

    /// Add a scrollbar element with a wider hit area for easier clicking.
    /// Visual bounds stay the same but the clickable area is padded to the left.
    pub fn add_scrollbar(&mut self, id: UiElementId, bounds: Rect) {
        const MIN_HIT_WIDTH: f32 = 20.0;
        let extra = (MIN_HIT_WIDTH - bounds.w).max(0.0);
        let hit_bounds = Rect::new(bounds.x - extra, bounds.y, bounds.w + extra, bounds.h);
        self.elements.push(UiElement {
            id,
            bounds: hit_bounds,
        });
    }

    /// Get bounds for a specific element
    pub fn get_bounds(&self, id: &UiElementId) -> Option<Rect> {
        self.elements.iter().find(|e| &e.id == id).map(|e| e.bounds)
    }

    /// Store max scroll value for a scrollbar element (set by renderer)
    pub fn set_max_scroll(&mut self, id: UiElementId, max_scroll: f32) {
        self.scroll_limits.push((id, max_scroll));
    }

    /// Get max scroll value for a scrollbar element (read by input handler)
    pub fn get_max_scroll(&self, id: &UiElementId) -> Option<f32> {
        self.scroll_limits
            .iter()
            .find(|(eid, _)| eid == id)
            .map(|(_, v)| *v)
    }

    /// Find element at mouse position (topmost - iterate in reverse)
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&UiElementId> {
        self.elements
            .iter()
            .rev()
            .find(|e| e.bounds.contains(Vec2::new(x, y)))
            .map(|e| &e.id)
    }
}
