import { openUrl } from "@tauri-apps/plugin-opener";
import type { ReactNode } from "react";

import { errorMessage } from "../../services/errors";
import { notify } from "../../store/ui";

/** Opens the link in the default browser instead of the app window. */
export function ExternalLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a
      href={href}
      onClick={(event) => {
        event.preventDefault();
        openUrl(href).catch((error: unknown) => notify(errorMessage(error), "error"));
      }}
      className="rounded text-accent underline decoration-accent/40 underline-offset-2 hover:decoration-accent"
    >
      {children}
    </a>
  );
}
