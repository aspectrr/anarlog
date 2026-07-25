import {
  type ReactNode,
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { cn } from "@hypr/utils";

import { useSearch } from "../../search/context";
import { useRenderedTranscriptData, useTranscriptOffset } from "./data-hooks";
import {
  EMPTY_TRANSCRIPT_SEARCH,
  SegmentRenderer,
  type TranscriptSearchRenderState,
} from "./segment";
import {
  createSegmentKey,
  segmentsShallowEqual,
  useStableSegments,
} from "./segment-hooks";

import {
  mergeRenderedAndLiveSegments,
  type Segment,
  type SegmentWord,
} from "~/stt/live-segment";
import { useTranscriptLabelContext } from "~/stt/queries";
import { SpeakerLabelManager } from "~/stt/segment/shared";
import { isTranscriptWordSeekable } from "~/stt/timing";

export function RenderTranscript({
  scrollElement,
  isLastTranscript,
  shouldScrollToEnd,
  transcriptId,
  liveSegments,
  currentMs,
  seek,
  startPlayback,
  audioExists,
}: {
  scrollElement: HTMLDivElement | null;
  isLastTranscript: boolean;
  shouldScrollToEnd: boolean;
  transcriptId: string;
  liveSegments: Segment[];
  currentMs: number;
  seek: (sec: number) => void;
  startPlayback: () => void;
  audioExists: boolean;
}) {
  const { maxSpeakerNumber, segments: storedSegments } =
    useRenderedTranscriptData(transcriptId);
  const mergedSegments = useMemo(
    () => mergeRenderedAndLiveSegments(storedSegments, liveSegments),
    [liveSegments, storedSegments],
  );
  const segments = useStableSegments(mergedSegments);
  const offsetMs = useTranscriptOffset(transcriptId);

  if (segments.length === 0) {
    return null;
  }

  return (
    <SegmentsList
      segments={segments}
      scrollElement={scrollElement}
      transcriptId={transcriptId}
      offsetMs={offsetMs}
      shouldScrollToEnd={isLastTranscript && shouldScrollToEnd}
      currentMs={currentMs}
      seek={seek}
      startPlayback={startPlayback}
      audioExists={audioExists}
      maxSpeakerNumber={maxSpeakerNumber}
    />
  );
}

const SegmentsList = memo(
  ({
    segments,
    scrollElement,
    transcriptId,
    offsetMs,
    shouldScrollToEnd,
    currentMs,
    seek,
    startPlayback,
    audioExists,
    maxSpeakerNumber,
  }: {
    segments: Segment[];
    scrollElement: HTMLDivElement | null;
    transcriptId: string;
    offsetMs: number;
    shouldScrollToEnd: boolean;
    currentMs: number;
    seek: (sec: number) => void;
    startPlayback: () => void;
    audioExists: boolean;
    maxSpeakerNumber?: number;
  }) => {
    const labelContext = useTranscriptLabelContext(transcriptId);
    const search = useSearch();
    const speakerLabelManager = useMemo(() => {
      return labelContext
        ? SpeakerLabelManager.fromSegments(
            segments,
            labelContext,
            maxSpeakerNumber,
          )
        : new SpeakerLabelManager();
    }, [labelContext, maxSpeakerNumber, segments]);
    const transcriptSearch = useMemo<TranscriptSearchRenderState>(() => {
      const query = search?.query.trim() ?? "";
      if (!search?.isVisible || !query) {
        return EMPTY_TRANSCRIPT_SEARCH;
      }

      return {
        query,
        activeMatchId: search.activeMatchId,
        caseSensitive: search.caseSensitive,
        wholeWord: search.wholeWord,
      };
    }, [
      search?.activeMatchId,
      search?.caseSensitive,
      search?.isVisible,
      search?.query,
      search?.wholeWord,
    ]);

    const seekAndPlay = useCallback(
      (word: SegmentWord) => {
        if (audioExists && isTranscriptWordSeekable(word)) {
          seek((offsetMs + word.start_ms) / 1000);
          startPlayback();
        }
      },
      [audioExists, offsetMs, seek, startPlayback],
    );

    useEffect(() => {
      if (!scrollElement || !shouldScrollToEnd) {
        return;
      }
      const raf = requestAnimationFrame(() => {
        scrollElement.scrollTo({
          top: scrollElement.scrollHeight,
          behavior: "auto",
        });
      });
      return () => cancelAnimationFrame(raf);
    }, [scrollElement, segments.length, shouldScrollToEnd]);

    // ponytail: search scans the live DOM ([data-word-id] spans) to count
    // matches and scroll to them, so it needs every segment mounted. Outside
    // search we window: only segments near the viewport mount their words,
    // otherwise a 15k-word transcript OOMs the WebView (~2.7GB WebKit cap).
    const windowed = !transcriptSearch.query;

    return (
      <div>
        {segments.map((segment, index) => {
          const key = createSegmentKey(segment, transcriptId, index);
          const rendered = (
            <SegmentRenderer
              segment={segment}
              offsetMs={offsetMs}
              transcriptId={transcriptId}
              speakerLabelManager={speakerLabelManager}
              currentMs={currentMs}
              seekAndPlay={seekAndPlay}
              audioExists={audioExists}
              search={transcriptSearch}
            />
          );

          if (!windowed) {
            return (
              <div key={key} className={cn([index > 0 && "pt-4"])}>
                {rendered}
              </div>
            );
          }

          return (
            <VirtualSegment
              key={key}
              itemKey={key}
              index={index}
              scrollElement={scrollElement}
              estimate={estimateSegmentHeight(segment)}
            >
              {rendered}
            </VirtualSegment>
          );
        })}
      </div>
    );
  },
  (prevProps, nextProps) => {
    return (
      prevProps.transcriptId === nextProps.transcriptId &&
      prevProps.scrollElement === nextProps.scrollElement &&
      prevProps.offsetMs === nextProps.offsetMs &&
      prevProps.shouldScrollToEnd === nextProps.shouldScrollToEnd &&
      prevProps.currentMs === nextProps.currentMs &&
      prevProps.audioExists === nextProps.audioExists &&
      prevProps.maxSpeakerNumber === nextProps.maxSpeakerNumber &&
      prevProps.seek === nextProps.seek &&
      prevProps.startPlayback === nextProps.startPlayback &&
      segmentsShallowEqual(prevProps.segments, nextProps.segments)
    );
  },
);

// ponytail: mount anything within this distance above OR below the viewport.
// Large enough that recent segments stay live when scrolling back up (no
// re-mount churn), small enough to bound total live DOM nodes on a long
// transcript. ~5000px ≈ 15–20 segments of headroom. Shrink if memory
// pressure returns on very large sessions.
const VIRTUAL_WINDOW_PX = 5000;
// Survives remounts so scroll position stays stable across transcript re-renders.
const segmentHeightCache = new Map<string, number>();

// ponytail: rough placeholder so off-screen segments reserve vertical space
// before their first measurement. Refined to exact height by ResizeObserver
// once mounted. Ceiling: wraps based on fixed chars-per-line; stays within a
// few % for typical transcript widths, scrollbar thumb nudges on correction.
function estimateSegmentHeight(segment: Segment): number {
  const HEADER_AND_GAP_PX = 52;
  const CHARS_PER_LINE = 55;
  const LINE_HEIGHT_PX = 24;
  const chars = segment.text?.length ?? segment.words.length * 6;
  const lines = Math.max(1, Math.ceil(chars / CHARS_PER_LINE));
  return HEADER_AND_GAP_PX + lines * LINE_HEIGHT_PX;
}

function VirtualSegment({
  itemKey,
  index,
  scrollElement,
  estimate,
  children,
}: {
  itemKey: string;
  index: number;
  scrollElement: HTMLElement | null;
  estimate: number;
  children: ReactNode;
}) {
  const supportsIntersectionObserver =
    typeof IntersectionObserver !== "undefined";
  const [inView, setInView] = useState(false);
  const [height, setHeight] = useState<number | null>(
    () => segmentHeightCache.get(itemKey) ?? null,
  );
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!supportsIntersectionObserver) {
      setInView(true);
      return;
    }
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        setInView(entry?.isIntersecting ?? false);
      },
      {
        root: scrollElement ?? null,
        rootMargin: `${VIRTUAL_WINDOW_PX}px 0px`,
      },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [scrollElement, supportsIntersectionObserver]);

  useEffect(() => {
    if (!inView) return;
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      const next = el.offsetHeight;
      setHeight((prev) => (prev === next ? prev : next));
      segmentHeightCache.set(itemKey, next);
    };
    measure();
    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(measure);
    observer?.observe(el);
    return () => observer?.disconnect();
  }, [inView, itemKey]);

  return (
    <div
      ref={ref}
      className={cn([index > 0 && "pt-4"])}
      style={!inView ? { height: height ?? estimate } : undefined}
    >
      {inView ? children : null}
    </div>
  );
}
