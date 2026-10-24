use borsh::{BorshDeserialize, BorshSerialize};
use deadroll_types::*;

fn pk(n: u8) -> Pubkey {
    Pubkey::new_from_array([n; 32])
}

/// Borsh round-trip; the encoding of `value` must be exactly `size` bytes.
fn check<T>(value: &T, size: usize) -> Vec<u8>
where
    T: BorshSerialize + BorshDeserialize + PartialEq + core::fmt::Debug,
{
    let bytes = borsh::to_vec(value).unwrap();
    assert_eq!(bytes.len(), size, "encoded size of {value:?}");
    let back = T::try_from_slice(&bytes).unwrap();
    assert_eq!(&back, value);
    bytes
}

fn last<T: Copy>(all: &[T]) -> T {
    *all.last().unwrap()
}

fn layer_max() -> LayerRef {
    LayerRef {
        sprite_id: u16::MAX,
        material_ramp: u8::MAX,
        accent_ramp: u8::MAX,
        element_ramp: u8::MAX,
    }
}

fn state_max() -> ActiveState {
    ActiveState {
        kind: last(StateKind::ALL),
        turns: u8::MAX,
        power: u32::MAX,
        source: u8::MAX,
    }
}

fn spawn_max() -> Spawn {
    Spawn {
        bestiary_id: u8::MAX,
        x: u8::MAX,
        y: u8::MAX,
        level: u8::MAX,
        skills: u16::MAX,
        flags: u8::MAX,
        aspects: u8::MAX,
        pack: u8::MAX,
    }
}

fn monster_max() -> Monster {
    Monster {
        bestiary_id: u8::MAX,
        level: u8::MAX,
        x: u8::MAX,
        y: u8::MAX,
        hp: u32::MAX,
        hp_max: u32::MAX,
        action_points: u8::MAX,
        alive: true,
        pack: u8::MAX,
        flags: u8::MAX,
        states: [state_max(); STATES_MAX],
    }
}

fn game_config(version: u8) -> GameConfigData {
    GameConfigData {
        version,
        authority: pk(1),
        programs: [pk(2); PROGRAMS_COUNT],
        rasterizer_version: u16::MAX,
        rasterizer: pk(3),
        rasterizer_sha256: [0xAB; 32],
        palette: pk(4),
        uri_base: [b'x'; URI_BASE_LEN],
        bump: u8::MAX,
    }
}

fn character(version: u8, nft: Option<Pubkey>, in_session: Option<Pubkey>) -> CharacterData {
    CharacterData {
        version,
        owner: pk(5),
        index: u8::MAX,
        nft,
        race: last(Race::ALL),
        class: last(Class::ALL),
        level: u16::MAX,
        xp: u32::MAX,
        stats: [u16::MAX; STATS_COUNT],
        predisposition: last(Aspect::ALL),
        predisposition_tier: Tier::WORST.get(),
        hp_max: u32::MAX,
        alive: true,
        in_session,
        appearance: [layer_max(); LAYERS_MAX],
        state_version: u32::MAX,
        bump: u8::MAX,
    }
}

fn map(version: u8) -> MapData {
    MapData {
        version,
        dm: pk(6),
        index: u32::MAX,
        width: MAP_MAX as u8,
        height: MAP_MAX as u8,
        status: last(MapStatus::ALL),
        layout_hash: [0xCD; 32],
        secrets_root: [0xEF; 32],
        bump: u8::MAX,
    }
}

fn map_layout(version: u8) -> MapLayoutData {
    MapLayoutData {
        version,
        map: pk(7),
        tiles: [Tile::Trap as u8; MAP_CELLS],
        spawns: [spawn_max(); SPAWNS_MAX],
        spawn_count: SPAWNS_MAX as u8,
        entrance: Cell { x: 0, y: 1 },
        exit: Cell { x: 31, y: 31 },
        bump: u8::MAX,
        chests: [point_max(); CHESTS_MAX],
        chest_count: CHESTS_MAX as u8,
        boss_table: u8::MAX,
    }
}

fn point_max() -> MapPoint {
    MapPoint {
        kind: last(PointKind::ALL),
        x: u8::MAX,
        y: u8::MAX,
        data: u8::MAX,
    }
}

fn cache_slot_max() -> CacheSlot {
    CacheSlot {
        kind: last(RewardKind::ALL),
        id: u16::MAX,
        qty: u16::MAX,
    }
}

fn game_session(version: u8) -> GameSessionData {
    GameSessionData {
        version,
        map: pk(8),
        dm: pk(9),
        status: last(SessionStatus::ALL),
        price_lamports: u64::MAX,
        players: [pk(10); PARTY_MAX],
        player_count: PARTY_MAX as u8,
        turn_order: [u8::MAX; TURN_ORDER_MAX],
        turn_index: u8::MAX,
        round: u16::MAX,
        action_counter: u32::MAX,
        pending_vrf: true,
        last_seed: [0x11; 32],
        bump: u8::MAX,
    }
}

fn participation(version: u8) -> ParticipationData {
    ParticipationData {
        version,
        game_session: pk(11),
        character: pk(12),
        session_key: pk(13),
        slot: u8::MAX,
        x: u8::MAX,
        y: u8::MAX,
        hp: u32::MAX,
        action_points: u8::MAX,
        states: [state_max(); STATES_MAX],
        ready: true,
        bump: u8::MAX,
    }
}

fn monster_state(version: u8) -> MonsterStateData {
    MonsterStateData {
        version,
        session: pk(14),
        monsters: [monster_max(); MONSTERS_MAX],
        count: MONSTERS_MAX as u8,
        bump: u8::MAX,
    }
}

macro_rules! enum_tests {
    ($($test:ident: $ty:ty, $count:expr;)*) => {$(
        #[test]
        fn $test() {
            assert_eq!(<$ty>::COUNT, $count);
            for (i, v) in <$ty>::ALL.iter().enumerate() {
                assert_eq!(u8::from(*v) as usize, i);
                assert_eq!(<$ty>::try_from(i as u8), Ok(*v));
                assert_eq!(check(v, <$ty>::SIZE), vec![i as u8]);
            }
            for bad in [<$ty>::COUNT as u8, u8::MAX] {
                assert_eq!(<$ty>::try_from(bad), Err(InvalidValue(bad)));
                assert!(<$ty>::try_from_slice(&[bad]).is_err());
            }
        }
    )*};
}

enum_tests! {
    aspect: Aspect, 6;
    family: Family, 4;
    race: Race, 2;
    class: Class, 3;
    source: Source, 2;
    form: Form, 2;
    time_tag: TimeTag, 2;
    range_tag: RangeTag, 2;
    state_kind: StateKind, 7;
    tile: Tile, 7;
    session_status: SessionStatus, 6;
    map_status: MapStatus, 4;
    stat: Stat, STATS_COUNT;
    program_idx: ProgramIdx, PROGRAMS_COUNT;
    point_kind: PointKind, 1;
    reward_kind: RewardKind, 5;
    reward_source: RewardSource, 4;
    cache_status: CacheStatus, 3;
}

#[test]
fn loot_structs() {
    check(&point_max(), MapPoint::SIZE);
    check(&cache_slot_max(), CacheSlot::SIZE);
    let bag = BagSlot {
        kind: last(RewardKind::ALL),
        id: u16::MAX,
        qty: u8::MAX,
    };
    check(&bag, BagSlot::SIZE);
    assert!(BagSlot::EMPTY.is_empty() && CacheSlot::EMPTY.is_empty());
    assert_eq!(RewardKind::Consumable as u8, 2);
    assert_eq!(RewardSource::Stash as u8, 3);
}

/// `Participation` keeps the cache as raw bytes: they must be the borsh bytes of the slots.
#[test]
fn raw_cache_is_borsh() {
    let mut slots = [CacheSlot::EMPTY; CACHE_SLOTS];
    slots[0] = cache_slot_max();
    slots[CACHE_SLOTS - 1] = CacheSlot {
        kind: RewardKind::Recipe,
        id: 0x0102,
        qty: 7,
    };
    let raw = CacheSlot::write_all(&slots);
    assert_eq!(raw.to_vec(), borsh::to_vec(&slots).unwrap());
    assert_eq!(CacheSlot::read_all(&raw), slots);
    let mut bad = raw;
    bad[0] = RewardKind::COUNT as u8;
    assert!(CacheSlot::read(&bad, 0).is_empty());
}

#[test]
fn layout_chest_list_is_bounded() {
    let mut l = map_layout(1);
    assert_eq!(l.chest_list().len(), CHESTS_MAX);
    l.chest_count = u8::MAX;
    assert_eq!(l.chest_list().len(), CHESTS_MAX);
    l.chest_count = 1;
    assert_eq!(l.chest_list(), &[point_max()]);
    assert_eq!(point_max().cell(), Cell { x: 255, y: 255 });
}

#[test]
fn enum_discriminants_match_contract_order() {
    assert_eq!(Aspect::Piercing as u8, 5);
    assert_eq!(Race::ForestElf as u8, 1);
    assert_eq!(StateKind::Bleeding as u8, 3);
    assert_eq!(StateKind::Penetration as u8, 6);
    assert_eq!(Tile::Trap as u8, 6);
    assert_eq!(SessionStatus::Abandoned as u8, 5);
    assert_eq!(MapStatus::Finished as u8, 3);
    assert_eq!(ProgramIdx::Session as usize, 5);
    assert_eq!(RangeTag::Ranged as u8, 1);
}

#[test]
fn tier_bounds() {
    assert_eq!(Tier::new(0), Err(InvalidValue(0)));
    assert_eq!(Tier::new(1).map(Tier::get), Ok(1));
    assert_eq!(Tier::new(14).map(Tier::get), Ok(14));
    assert_eq!(Tier::new(15), Err(InvalidValue(15)));
    assert_eq!(Tier::try_from(u8::MAX), Err(InvalidValue(u8::MAX)));
    assert!(Tier::BEST < Tier::WORST);
}

#[test]
fn small_structs() {
    check(&layer_max(), LayerRef::SIZE);
    check(&LayerRef::EMPTY, LayerRef::SIZE);
    assert!(LayerRef::EMPTY.is_empty() && !layer_max().is_empty());
    check(&state_max(), ActiveState::SIZE);
    assert!(ActiveState::default().is_empty());
    check(&spawn_max(), Spawn::SIZE);
    check(&Cell { x: 3, y: 4 }, Cell::SIZE);
    check(&monster_max(), Monster::SIZE);
    check(&Monster::default(), Monster::SIZE);
    let request = RenderRequest {
        palette_version: u16::MAX,
        layers: [layer_max(); LAYERS_MAX],
        flags: u32::MAX,
    };
    check(&request, RenderRequest::SIZE);
}

#[test]
fn account_sizes_and_version_first() {
    let v = 0x7E;
    assert_eq!(check(&game_config(v), GameConfigData::SIZE)[0], v);
    assert_eq!(check(&map(v), MapData::SIZE)[0], v);
    assert_eq!(check(&map_layout(v), MapLayoutData::SIZE)[0], v);
    assert_eq!(check(&game_session(v), GameSessionData::SIZE)[0], v);
    assert_eq!(check(&participation(v), ParticipationData::SIZE)[0], v);
    assert_eq!(check(&monster_state(v), MonsterStateData::SIZE)[0], v);
    let full = character(v, Some(pk(20)), Some(pk(21)));
    assert_eq!(check(&full, CharacterData::SIZE)[0], v);
}

#[test]
fn character_options_shrink_encoding_but_size_is_max() {
    let empty = character(1, None, None);
    let bytes = borsh::to_vec(&empty).unwrap();
    assert_eq!(bytes.len(), CharacterData::SIZE - 2 * 32);
    assert_eq!(CharacterData::try_from_slice(&bytes).unwrap(), empty);
}

#[test]
fn owner_follows_version() {
    // RPC memcmp filters rely on these offsets (8-byte discriminator not included).
    let c = character(1, None, None);
    assert_eq!(&borsh::to_vec(&c).unwrap()[1..33], pk(5).as_ref());
    let m = map(1);
    assert_eq!(&borsh::to_vec(&m).unwrap()[1..33], pk(6).as_ref());
}

#[test]
fn invalid_enum_byte_in_account_is_rejected() {
    let mut bytes = borsh::to_vec(&map(1)).unwrap();
    let status_offset = 1 + 32 + 4 + 1 + 1;
    assert_eq!(bytes[status_offset], MapStatus::Finished as u8);
    bytes[status_offset] = MapStatus::COUNT as u8;
    assert!(MapData::try_from_slice(&bytes).is_err());
}

#[test]
fn seeds_match_contract() {
    assert_eq!(seeds::CONFIG, b"config");
    assert_eq!(seeds::AUTHORITY, b"authority");
    assert_eq!(seeds::CHARACTER, b"character");
    assert_eq!(seeds::PARTICIPATION, b"participation");
    assert_eq!(seeds::MONSTERS, b"monsters");
    assert_eq!(seeds::REWARDS, b"rewards");
    assert_eq!(seeds::CACHE, b"cache");
}

#[cfg(feature = "anchor")]
#[test]
fn size_matches_anchor_init_space() {
    use anchor_lang::Space;
    assert_eq!(LayerRef::SIZE, LayerRef::INIT_SPACE);
    assert_eq!(RenderRequest::SIZE, RenderRequest::INIT_SPACE);
    assert_eq!(Cell::SIZE, Cell::INIT_SPACE);
    assert_eq!(ActiveState::SIZE, ActiveState::INIT_SPACE);
    assert_eq!(Spawn::SIZE, Spawn::INIT_SPACE);
    assert_eq!(MapPoint::SIZE, MapPoint::INIT_SPACE);
    assert_eq!(CacheSlot::SIZE, CacheSlot::INIT_SPACE);
    assert_eq!(BagSlot::SIZE, BagSlot::INIT_SPACE);
    assert_eq!(Monster::SIZE, Monster::INIT_SPACE);
    assert_eq!(Aspect::SIZE, Aspect::INIT_SPACE);
    assert_eq!(GameConfigData::SIZE, GameConfigData::INIT_SPACE);
    assert_eq!(CharacterData::SIZE, CharacterData::INIT_SPACE);
    assert_eq!(MapData::SIZE, MapData::INIT_SPACE);
    assert_eq!(MapLayoutData::SIZE, MapLayoutData::INIT_SPACE);
    assert_eq!(GameSessionData::SIZE, GameSessionData::INIT_SPACE);
    assert_eq!(ParticipationData::SIZE, ParticipationData::INIT_SPACE);
    assert_eq!(MonsterStateData::SIZE, MonsterStateData::INIT_SPACE);
    assert_eq!(ItemRecord::SIZE, ItemRecord::INIT_SPACE);
    assert_eq!(EquipRef::SIZE, EquipRef::INIT_SPACE);
    assert_eq!(LoadoutData::SIZE, LoadoutData::INIT_SPACE);
    assert_eq!(skill::SkillBlocks::SIZE, skill::SkillBlocks::INIT_SPACE);
    assert_eq!(skill::SkillExt::SIZE, skill::SkillExt::INIT_SPACE);
}

/// Moved here from `crates/skillcheck` (patch 14): the bytes stored in character and session
/// accounts must stay exactly these.
#[test]
fn skill_blocks_bytes() {
    use skill::*;
    let blocks = SkillBlocks {
        category: Category::Reactive,
        purpose: Purpose::Debuff,
        source: Source::Combat,
        form: Form::Projectile,
        target: Target::Line,
        aspects: [Aspect::Cold, Aspect::Piercing, Aspect::Fire],
        aspect_count: 3,
        effect: last(Effect::ALL),
        trigger: last(Trigger::ALL),
        aura_radius: 2,
    };
    let bytes = check(&blocks, SkillBlocks::SIZE);
    assert_eq!(bytes, [1, 2, 1, 1, 1, 1, 5, 0, 3, 10, 6, 2]);

    let ext = SkillExt {
        range: RangeTag::Ranged,
        timing: TimeTag::DoT,
        hp_below_pct: 40,
    };
    assert_eq!(check(&ext, SkillExt::SIZE), [1, 1, 40]);
    assert_eq!(check(&SkillExt::MELEE, SkillExt::SIZE), [0, 0, 0]);

    assert_eq!(Category::COUNT, 3);
    assert_eq!(Purpose::COUNT, 3);
    assert_eq!(Target::COUNT, 2);
    assert_eq!(Effect::COUNT, 11);
    assert_eq!(Trigger::COUNT, 7);
}

fn loadout(version: u8) -> LoadoutData {
    LoadoutData {
        version,
        character: pk(15),
        owner: pk(16),
        items: [ItemRecord {
            base: last(ItemBase::ALL),
            level: u8::MAX,
        }; SOULBOUND_MAX],
        item_count: SOULBOUND_MAX as u8,
        equipped: [EquipRef {
            source: last(EquipSource::ALL),
            index: u8::MAX,
            asset: pk(17),
        }; EQUIP_SLOTS],
        bump: u8::MAX,
    }
}

enum_tests! {
    equip_slot: EquipSlot, EQUIP_SLOTS;
    item_base: ItemBase, 6;
    equip_source: EquipSource, 3;
}

#[test]
fn loadout_size_version_and_owner_offsets() {
    let v = 0x7E;
    let bytes = check(&loadout(v), LoadoutData::SIZE);
    assert_eq!(bytes[0], v);
    assert_eq!(&bytes[1..33], pk(15).as_ref());
    assert_eq!(&bytes[33..65], pk(16).as_ref());
    check(&EquipRef::EMPTY, EquipRef::SIZE);
    assert_eq!(seeds::LOADOUT, b"loadout");
}

#[test]
fn loadout_equipment_reads_worn_soulbound_bases() {
    let mut l = loadout(1);
    l.items[0].base = ItemBase::Sword;
    l.items[1].base = ItemBase::LeatherArmor;
    l.item_count = 2;
    l.equipped = [EquipRef::EMPTY; EQUIP_SLOTS];
    l.equipped[EquipSlot::MainHand as usize] = EquipRef::soulbound(0);
    l.equipped[EquipSlot::Body as usize] = EquipRef::soulbound(1);
    // Past `item_count`: ignored.
    l.equipped[EquipSlot::Ring1 as usize] = EquipRef::soulbound(3);
    let eq = l.equipment();
    assert_eq!(eq[EquipSlot::MainHand as usize], ItemBase::Sword as u8);
    assert_eq!(eq[EquipSlot::Body as usize], ItemBase::LeatherArmor as u8);
    assert_eq!(eq[EquipSlot::Ring1 as usize], EMPTY_ITEM);
    assert_eq!(eq.iter().filter(|b| **b != EMPTY_ITEM).count(), 2);
    assert_eq!(l.worn_slot(1), Some(EquipSlot::Body));
    assert_eq!(l.worn_slot(2), None);
}
