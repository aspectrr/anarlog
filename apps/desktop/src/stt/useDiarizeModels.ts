import { queryOptions } from "@tanstack/react-query";
import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useState } from "react";

import {
  commands as transcriptionCommands,
  events as transcriptionEvents,
} from "@hypr/plugin-transcription";

export const diarizeKeys = {
  all: ["diarize"] as const,
  downloaded: () => [...diarizeKeys.all, "downloaded"] as const,
  size: () => [...diarizeKeys.all, "size"] as const,
};

export const diarizeQueries = {
  isDownloaded: () =>
    queryOptions({
      queryKey: diarizeKeys.downloaded(),
      queryFn: () => transcriptionCommands.isDiarizeModelsDownloaded(),
      select: (result) => {
        if (result.status === "error") {
          throw new Error(result.error);
        }
        return result.data;
      },
    }),
  sizeBytes: () =>
    queryOptions({
      queryKey: diarizeKeys.size(),
      queryFn: () => transcriptionCommands.diarizeModelsSizeBytes(),
      select: (result) => {
        if (result.status === "error") {
          throw new Error(result.error);
        }
        return result.data;
      },
    }),
};

export function useDiarizeModels() {
  const [progress, setProgress] = useState(0);
  const [isDownloading, setIsDownloading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const isDownloaded = useQuery(diarizeQueries.isDownloaded());
  const sizeQuery = useQuery(diarizeQueries.sizeBytes());

  useEffect(() => {
    const unlisten = transcriptionEvents.diarizeDownloadEvent.listen(
      (event) => {
        switch (event.payload.type) {
          case "progress":
            setProgress(event.payload.percentage);
            setErrorMessage(null);
            break;
          case "completed":
            setProgress(0);
            setIsDownloading(false);
            setErrorMessage(null);
            void isDownloaded.refetch();
            break;
          case "failed":
            setProgress(0);
            setIsDownloading(false);
            setErrorMessage(event.payload.error);
            break;
        }
      },
    );

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [isDownloaded]);

  const handleDownload = useCallback(() => {
    if (isDownloading || isDownloaded.data) {
      return;
    }
    setErrorMessage(null);
    setProgress(0);
    setIsDownloading(true);
    void transcriptionCommands.downloadDiarizeModels().then((result) => {
      if (result.status === "error") {
        setErrorMessage(result.error);
        setIsDownloading(false);
      }
    });
  }, [isDownloading, isDownloaded.data]);

  const handleDelete = useCallback(() => {
    void transcriptionCommands.deleteDiarizeModels().then((result) => {
      if (result.status === "ok") {
        void isDownloaded.refetch();
      }
    });
  }, [isDownloaded]);

  return {
    isDownloaded: isDownloaded.data ?? false,
    isDownloading,
    progress,
    errorMessage,
    hasError: errorMessage !== null,
    sizeBytes: sizeQuery.data,
    handleDownload,
    handleDelete,
  };
}
