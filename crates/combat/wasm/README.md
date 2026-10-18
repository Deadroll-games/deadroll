# deadroll-combat-wasm

C ABI over `crates/combat` for the website. The page uses it to replay any hit from the VRF seed in a `HitResolved` event and get the same breakdown as the chain, to preview an attack before the dice (chance to hit, chance to crit, damage range; docs/COMBAT_MATH.md, section 13), and to show a hero's derived stats. The preview and the hero sheet build their inputs with `crates/rules`, the code the session program runs, so the website repeats no formula.

## Build

The website ships the built module as `app/src/chain/wasm/combat.wasm` (Vercel builds without Rust). Rebuild it after any change in `crates/combat`, `crates/rules`, `crates/skillcheck`, `crates/types` or this crate, and commit it:

```
bash crates/combat/wasm/build.sh            # needs Docker; writes app/src/chain/wasm/combat.wasm
bash crates/combat/wasm/build.sh --check    # what CI runs: fails if the committed file differs
```

The script builds in the `rust:1.92.0` image so the bytes are the same on every machine (the module embeds source paths in its panic locations). A plain local build is fine for development but differs in bytes:

```
cargo build -p deadroll-combat-wasm --target wasm32-unknown-unknown --profile wasm
# -> target/wasm32-unknown-unknown/wasm/deadroll_combat_wasm.wasm
```

There are no imports. The module exports `memory` plus the functions below. Balance numbers are always `BalanceConfig::DEFAULT`, the numbers the programs use. Resonance tables and monster defenses are registry data, not compiled in: the page loads the tables a session copied at `open_session` (`MonsterState.res_aspects ‖ res_traits ‖ res_pairs`, 816 bytes) with `set_tables` before `resolve`, `preview_hit` or `preview_attack`, and passes each monster's `MonsterState.defense`.

## Exports

| Export | Signature | What it does |
|---|---|---|
| `abi_version` | `() -> u32` | `5`; bumped on any change of exports or layouts |
| `buffer_len` | `() -> u32` | size of the input and output buffers (4096) |
| `input_ptr` | `() -> *mut u8` | where to write the borsh input |
| `seed_ptr` | `() -> *mut u8` | where to write the 32-byte VRF seed |
| `output_ptr` | `() -> *const u8` | where results are read from |
| `set_tables` | `(input_len: u32) -> i32` | the 816 table bytes in: decodes and keeps them, returns 0 (or `-5`, then none are kept) |
| `resolve` | `(input_len: u32) -> i32` | `HitInput` in: `resolve_hit(input, DEFAULT, tables, VrfRoll(seed))`, writes a borsh `HitResult`, returns its length |
| `preview` | `(input_len: u32) -> i32` | `HitInput` in: writes `min: u32 LE, max: u32 LE`, returns 8 |
| `preview_hit` | `(input_len: u32) -> i32` | `HitInput` in: writes a borsh `HitPreview` |
| `hero_sheet` | `(input_len: u32) -> i32` | `HeroInput` in: writes a borsh `HeroSheet` |
| `preview_attack` | `(input_len: u32) -> i32` | `AttackRequest` in: builds the hit input as the session program does, writes a borsh `HitPreview` |
| `monster_sheet` | `(input_len: u32) -> i32` | `MonsterInput` in: the template at a level with its registry defenses, writes a borsh `MonsterSheet` |
| `unit_mods` | `(input_len: u32) -> i32` | `UnitModsRequest` in: a unit's own buffs and debuffs with the auras over it (`deadroll_rules::aura::AuraField`), writes `[TimedMod; 6]` |
| `line` | `(input_len: u32) -> i32` | `LineRequest` in: the cells of a line target, nearest first (`deadroll_rules::line::line_cells`), writes a `LineOutput` |
| `roll_u64` | `(index: u32) -> u64` (BigInt) | roll number `index` of the seed: first 8 bytes LE of `sha256(seed ‖ index u32 LE)` |

Negative returns: `-1` input longer than the buffer, `-2` input is not valid borsh of the expected type (trailing bytes count as invalid), `-3` output does not fit, `-4` the target's bestiary id has no template, `-5` no resonance tables (call `set_tables`), or they do not decode.

```ts
const { instance } = await WebAssembly.instantiate(bytes, {});
const x = instance.exports as any;
const mem = () => new Uint8Array(x.memory.buffer);
mem().set(inputBytes, x.input_ptr());
mem().set(seed, x.seed_ptr());
const len = x.resolve(inputBytes.length);
if (len < 0) throw new Error(`combat wasm error ${len}`);
const resultBytes = mem().slice(x.output_ptr(), x.output_ptr() + len);
```

## Byte layout (borsh)

Borsh rules: integers are little endian; `bool` is 1 byte; fixed arrays have no length prefix; `Option<T>` is a `0`/`1` tag followed by `T`; an enum is a `u8` variant index followed by its fields. Enums from `crates/types` (`Aspect`, `StateKind`, `Source`, `Form`, `Family`) are one `u8` (CONTRACTS.md, section 3).

### `HitInput` = `Attacker`, `Defender`

```
Attacker {
  slot: u8, attack_bonus: u32, crit_chance_pct: u32,          // crit chance, percent; 0 = natural 20 only
  weapon: DamageRange,
  skill: Skill,
  added: [DamageRange; 3],
  increased: [Mod; 8],
  more: [Mod; 4],
  weaken_bp: u32, penetration_bp: u32, state_chance_bonus_bp: u32,
  tints: [Tint; 4],
  tempo_bonus_bp: u32,                                          // Dexterity increase on Chill
}
DamageRange { aspect: Aspect, min: u32, max: u32 }            // min == max == 0: empty slot
Skill {
  aspects: [Aspect; 3], aspect_count: u8,                      // 0 = basic attack (weapon aspect)
  shares_bp: [u32; 3], power_bp: u32, source: Source, form: Form, state_chance_pct: u32,
}
Mod { filter: ModFilter, bp: u32 }                             // bp == 0: empty slot
ModFilter = 0 Any | 1 Aspect(Aspect) | 2 Family(Family) | 3 Source(Source) | 4 Form(Form)
          | 5 SourceForm(Source, Form)
Tint { aspect: Aspect, tier: u8, carrier: Carrier }            // tier outside 1..=14: empty slot
Carrier = 0 Race | 1 Class | 2 Item

Defender {
  defense: u32, armor: u32,
  resist_bp: [u32; 6], vuln_bp: [u32; 6],                      // indexed by Aspect
  immune_aspects: u8, immune_states: u8,                        // bit 1 << Aspect / 1 << StateKind
  state_resist_bp: [u32; 7],                                    // indexed by StateKind (Penetration is 6)
  states: [ActiveState; 6],
}
ActiveState { kind: StateKind, turns: u8, power: u32, source: u8 }   // turns == 0: empty slot
```


### `HitPreview`

```
HitPreview {
  immune: bool,                                                 // immune to every packet: all zero
  hit_bp: u32,                                                  // d20 faces that hit, out of 20
  crit_bp: u32,                                                 // natural 20 + plain hits x crit roll
  min: u32, max: u32,                                           // damage of a hit, no resonance
  crit_min: u32, crit_max: u32,                                 // the same with the crit multiplier
  tints: [PreviewTint; 12], tint_count: u8,                     // bases in skill order, tints in slot order
}
PreviewTint {
  base: Aspect, tint: Aspect, carrier: Carrier, tier: u8,
  pure: bool,                                                   // tint == base: no roll, native state stronger
  chance_pct: u32, id: u8, generated: bool, condition_met: bool,
  min: u32, max: u32,                                           // damage if this tint alone triggers
}
```

### `HeroInput`, `HeroSheet`

```
HeroInput {                                                     // as the chain stores the character
  race: Race, class: Class, level: u16,
  stats: [u16; 4],                                              // Strength, Dexterity, Intelligence, Endurance
  predisposition: Aspect, predisposition_tier: u8,              // race trait
  class_tier: u8,                                               // Character.class_tier
  equipment: [u8; 11],                                          // ItemBase by EquipSlot, 0xFF = nothing
}
HeroSheet {
  accuracy: u32, evasion: u32, armor: u32, crit_bp: u32, hp_max: u32,
  action_points: u8, initiative: u16,
  martial_strike_bp: u32, martial_projectile_bp: u32, magical_bp: u32,    // damage increase
  martial_state_bp: u32, magical_state_bp: u32, tempo_state_bp: u32,      // state chance increase
  self_buff_turns: u8,
  crit_roll_bp: u32,                                            // d100 crit chance in percent, 0 = none
}
```

### `AttackRequest`

```
AttackRequest {
  hero: HeroInput,
  hero_mods: [TimedMod; 6],                                     // the attacker's buffs and debuffs
  skill: Option<SkillInput>,                                    // None = the weapon's basic attack
  target: TargetInput,
  accuracy: AccuracyBuff,                                       // Participation.accuracy
  party: PartyInput,                                            // ABI 5: the auras and the players' cells
  slot: u8,                                                     // the attacker's player slot
  target_at: Cell,                                              // the target's cell
}
Cell { x: u8, y: u8 }
PartyInput { auras: [[AuraInput; 2]; 4], cells: [Cell; 4], alive: u8 }   // GameSession.auras, player_pos, alive_mask
AuraInput { kind: ModKind, aspect: Aspect, power_bp: u32, radius: u8, turns: u8, hostile: bool, passive: bool, self_only: bool }  // turns == 0: none
UnitModsRequest { party: PartyInput, mods: [TimedMod; 6], monster: bool, slot: u8, at: Cell, basic_attack: bool }
LineRequest { from: Cell, through: Cell, reach: u8, width: u8, height: u8, blocked: [u8; 128] }  // bit y * 32 + x: impassable
LineOutput { len: u8, cells: [Cell; 32] }
AccuracyBuff { aspect: Aspect, turns: u8, bonus: u32 }          // turns == 0: none
SkillInput { blocks: SkillBlocks, shares_bp: [u32; 3], power_bp: u32 }  // SkillRecord without the name
TargetInput { bestiary_id: u8, level: u8, states: [ActiveState; 6], mods: [TimedMod; 6], defense: DefenseInput }
DefenseInput { resist: u8, immune_aspects: u8, immune_states: u8 }   // MonsterState.defense: bits
MonsterInput { bestiary_id: u8, level: u8, defense: DefenseInput }
MonsterSheet {
  weapon: DamageRange, attack_bonus: u32, evasion: u32, armor: u32,
  resist_bp: [u32; 6], immune_aspects: u8, immune_states: u8, resist_cap_bp: u32,
}
TimedMod { kind: ModKind, aspect: Aspect, turns: u8, power_bp: u32, source: u8 }   // turns == 0: empty
SkillBlocks {
  category: u8, purpose: u8, source: Source, form: Form, target: u8,
  aspects: [Aspect; 3], aspect_count: u8, effect: u8, trigger: u8, aura_radius: u8,
}
```

### `HitResult`

```
HitResult {
  outcome: u8 (0 Miss, 1 Hit, 2 Immune),
  d20: u8, attack_total: u32, defense: u32,
  crit: bool, crit_roll: Option<u16>,
  tint_rolls: [TintRoll; 12], tint_roll_count: u8,
  resonances: [Option<Resonance>; 3],                           // per skill base slot
  d100: u8,
  packets: [PacketResult; 6], packet_count: u8,
  resonance_states: [StateRoll; 3],                             // per skill base slot
  states: [ActiveState; 6],                                     // target after the hit
  damage: u32,
  rolls_used: u32,
  res_ops: [[OpResult; 6]; 3],                                  // per base slot, kind None after the used ones
  jumps: [Jump; 4], jump_count: u8,                             // states the program passes on (ResonanceSpread)
  knockback: u8, ignite_power: u32,
  target_mods: [TimedMod; 3], target_mod_count: u8,
  accuracy_buff: AccuracyBuff,
  landed: u8,                                                   // bit 1 << StateKind of what landed
}
OpResult {
  kind: u8 (deadroll_combat::tables::OpKind), crit_only: bool, arg: u8,
  origin: u8 (0 Pair, 1 Template, 2 Trait), tint: Aspect, carrier: Carrier, tier: u8,
  acted: bool, state: StateRoll, amount: u32, amount_max: u32,
}
Jump { mode: u8 (0 Adjacent, 1 Chain, 2 Line), state: ActiveState, filter: StateKind, limit: u8, base: Aspect }
TintRoll { base: Aspect, tint: Aspect, carrier: Carrier, tier: u8, chance_pct: u32, roll: u16, success: bool }
Resonance {
  base: Aspect, tint: Aspect, tier: u8, carrier: Carrier, id: u8, active: bool,
  second_tint: Option<Aspect>, second_tier: u8, second_carrier: Carrier, generated: bool,
}
PacketResult {
  aspect: Aspect, immune: bool,
  base_min: u64, base_max: u64,                                 // scaled x10_000
  increased_bp: u32, more_bp: u32, weaken_bp: u32,
  min: u64, max: u64, rolled: u64,                              // scaled x10_000
  armor: u32, resist_bp: u32, vuln_bp: u32,
  after_defense: u64,                                           // scaled x10_000
  state: StateRoll,
  pen_attacker_bp: u32, pen_state_bp: u32, vuln_shock_bp: u32,
}
StateRoll { kind: StateKind, rolled: bool, chance_pct: u32, roll: u16, success: bool, applied: Applied }
Applied = 0 None | 1 Added | 2 Refreshed | 3 Froze
```

Chance rolls (crit, tint, state) are a d100, `0..=100`; success is `roll < chance_pct`. Packet values are scaled by 10 000 (`5 damage == 50_000`). `damage` is the final rounded value. The resonance `id` is `base * 6 + tint`, which the UI uses to look up the name. The roll order is documented in `crates/combat/src/hit.rs`. `crates/combat/tests/golden.rs` holds the reference hits, including raw `roll_u64` values to check a TS replay against.
