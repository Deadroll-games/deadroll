// Anchor discriminators: first 8 bytes of sha256("<namespace>:<Name>"). Pinned so a rename
// cannot drift quietly; disc.test.ts recomputes them. Replace with sdk/ constants when L8 lands.
import { fromHex } from "./bytes";

export const ACCOUNT_DISC = {
  GameConfig: fromHex("2d929221aa456085"),
  Character: fromHex("8c73a524f1996654"),
  RasterizerCode: fromHex("682f12179ed67b2e"),
  Palette: fromHex("053a00b2c1b59c89"),
  SpriteBook: fromHex("ddbbcfb6275cbc05"),
  SessionTokenV2: fromHex("b20355fe0d748029"),
};

export const IX_DISC = {
  freeze_tombstone: fromHex("9f253aa1ccda438d"),
  create_session_v2: fromHex("dfe96c0741c2eb26"),
  revoke_session_v2: fromHex("d33b7dbc2b9b0866"),
};

export const EVENT_DISC = {
  CharacterDied: fromHex("008fbcc2de55c1ea"),
  TombstoneFrozen: fromHex("1df11ff4606c24f8"),
};
