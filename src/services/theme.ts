/** GNOME's accent colors, as libadwaita draws them. */
const GNOME_ACCENTS = {
  blue: "#3584e4",
  teal: "#2190a4",
  green: "#3a944a",
  yellow: "#c88800",
  orange: "#ed5b00",
  red: "#e62d42",
  pink: "#d56199",
  purple: "#9141ac",
  slate: "#6f8396",
} as const;

/** The color GNOME draws an accent color name in. */
export function accentHex(name: string): string {
  return (GNOME_ACCENTS as Record<string, string | undefined>)[name] ?? GNOME_ACCENTS.slate;
}

/** "blue" becomes "Blue". */
export function accentLabel(name: string): string {
  return name.charAt(0).toUpperCase() + name.slice(1);
}

/** GNOME's style setting in words. */
export function colorSchemeLabel(scheme: string): string {
  switch (scheme) {
    case "prefer-dark":
      return "Dark";
    case "prefer-light":
      return "Light";
    default:
      return "Default";
  }
}
