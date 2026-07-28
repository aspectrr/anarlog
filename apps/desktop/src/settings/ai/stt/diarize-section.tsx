import { Trans } from "@lingui/react/macro";
import { Check, Loader2, Trash2, Users } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useDiarizeModels } from "~/stt/useDiarizeModels";

export function DiarizeModelSection() {
  const {
    isDownloaded,
    isDownloading,
    progress,
    errorMessage,
    sizeBytes,
    handleDownload,
    handleDelete,
  } = useDiarizeModels();

  const sizeLabel = sizeBytes
    ? `${(sizeBytes / 1_000_000).toFixed(0)} MB`
    : null;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-0.5">
          <div className="flex items-center gap-2">
            <Users className="text-muted-foreground size-4" />
            <span className="text-sm font-medium">
              <Trans>Speaker identification</Trans>
            </span>
            {isDownloaded && <Check className="size-3.5 text-green-600" />}
          </div>
          <p className="text-muted-foreground text-xs">
            <Trans>
              Identifies and labels different speakers in local transcriptions.
            </Trans>
          </p>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          {isDownloaded ? (
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground hover:text-destructive"
              onClick={handleDelete}
            >
              <Trash2 className="size-3.5" />
              <Trans>Delete</Trans>
            </Button>
          ) : isDownloading ? (
            <div className="flex items-center gap-2">
              <div className="bg-muted h-1.5 w-24 overflow-hidden rounded-full">
                <div
                  className="bg-primary h-full transition-all duration-300"
                  style={{ width: `${progress}%` }}
                />
              </div>
              <span className="text-muted-foreground text-xs tabular-nums">
                {progress}%
              </span>
            </div>
          ) : (
            <Button variant="outline" size="sm" onClick={handleDownload}>
              {sizeLabel && (
                <span className="text-muted-foreground font-mono text-[11px]">
                  {sizeLabel}
                </span>
              )}
              <Trans>Download</Trans>
            </Button>
          )}
        </div>
      </div>

      {isDownloading && (
        <div className="text-muted-foreground flex items-center gap-1.5 text-xs">
          <Loader2 className="size-3 animate-spin" />
          <Trans>Downloading speaker models…</Trans>
        </div>
      )}

      {errorMessage && (
        <p className="text-destructive text-xs">{errorMessage}</p>
      )}
    </div>
  );
}
