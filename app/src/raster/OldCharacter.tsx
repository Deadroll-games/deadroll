/** Shown instead of the portrait of a character created before the current graphics (patch 7c). */
export const OLD_CHARACTER = "Old character version";

export function OldCharacterLabel() {
  return (
    <span className="old-character muted" role="note">
      {OLD_CHARACTER}
    </span>
  );
}
