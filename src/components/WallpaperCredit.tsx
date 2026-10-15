import type { Credit, Wallpaper } from "../types";
import { ExternalLink } from "./ui/ExternalLink";

interface WallpaperCreditProps {
  wallpaper: Wallpaper;
  providerName: string;
  /** Linked when the wallpaper has no page of its own. */
  providerWebsite: string | null;
}

/**
 * The attribution providers and licenses ask for: the author, linked to their
 * profile, and the provider, linked to the wallpaper's page.
 */
export function WallpaperCredit({ wallpaper, providerName, providerWebsite }: WallpaperCreditProps) {
  const { author, authorUrl, sourceUrl } = wallpaper.credit;
  const providerLink = sourceUrl ?? providerWebsite;
  const provider = providerLink ? <ExternalLink href={providerLink}>{providerName}</ExternalLink> : providerName;

  return (
    <p data-selectable className="mt-1 text-muted">
      {author ? (
        <>
          Photo by {authorUrl ? <ExternalLink href={authorUrl}>{author}</ExternalLink> : author} on {provider}
        </>
      ) : (
        <>From {provider}</>
      )}
    </p>
  );
}

/** The license, linked to its text, or null if the provider names none. */
export function LicenseName({ credit }: { credit: Credit }) {
  if (!credit.license) return null;
  return credit.licenseUrl ? <ExternalLink href={credit.licenseUrl}>{credit.license}</ExternalLink> : <>{credit.license}</>;
}
