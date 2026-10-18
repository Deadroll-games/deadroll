// The patch 5 blocks of the starter skills (`deadroll_skillcheck::presets`): range, timing, the HP
// threshold of a reactive trigger. Shared by the integration tests, which spell out the blocks.
const e = (name: string) => ({ [name]: {} });

/** Skills of the starter sets that reach far (DAMAGE_TYPES_AND_MIXES 2, the range axis). */
const RANGED = new Set(["Javelin", "Bleeding Cleave", "Frost Bolt", "Frost Burn", "Ice Prison", "Arc", "Quicken", "Sap", "Expose"]);
/** HP thresholds of the starter reactives, percent. */
const HP_BELOW: Record<string, number> = { "Last Stand": 40 };

/** `SkillExt` of a starter skill by name (Anchor TS shape). */
export function extOf(name: string, over: { range?: "melee" | "ranged"; hpBelowPct?: number } = {}) {
  return {
    range: e(over.range ?? (RANGED.has(name) ? "ranged" : "melee")),
    timing: e("instant"),
    hpBelowPct: over.hpBelowPct ?? HP_BELOW[name] ?? 0,
  };
}

/** A starter skill as `create_character` takes it (`NewSkill`). */
export function newSkill<B>(name: string, blocks: B, text: (s: string, len: number) => number[], ext = extOf(name)) {
  return { blocks: structuredClone(blocks), name: text(name, 24), ext };
}

/**
 * The accounts every VRF callback gets after its fixed list (programs/session party.rs): each
 * living player but `main`, in slot order, its Participation (writable) and its Character.
 * `s` is a decoded `GameSession`. `deaths` (patch 6, a monster can hit several players): the
 * Character writable, then its SkillSet and Bag (`deaths(character)`), so any of them can die.
 */
export function partyAccounts(
  s: { core: { playerCount: number; players: { equals(o: unknown): boolean }[] }; aliveMask: number; participations: { equals(o: unknown): boolean }[] },
  main: { equals(o: unknown): boolean },
  deaths: ((character: any) => [any, any]) | null = null,
) {
  const out: { pubkey: any; isSigner: boolean; isWritable: boolean }[] = [];
  for (let slot = 0; slot < s.core.playerCount; slot++) {
    const participation = s.participations[slot];
    if ((s.aliveMask & (1 << slot)) === 0 || participation.equals(main)) continue;
    out.push({ pubkey: participation, isSigner: false, isWritable: true });
    out.push({ pubkey: s.core.players[slot], isSigner: false, isWritable: deaths !== null });
    if (deaths) for (const pubkey of deaths(s.core.players[slot])) out.push({ pubkey, isSigner: false, isWritable: true });
  }
  return out;
}
