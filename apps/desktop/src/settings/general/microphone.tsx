import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import { commands as transcriptionCommands } from "@hypr/plugin-transcription";

import {
  SearchableSelect,
  type SearchableSelectOption,
} from "./searchable-select";

import { useSetSettingValue } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

const SYSTEM_DEFAULT = "";

export function MicrophoneSelector() {
  const { t } = useLingui();
  const value = useConfigValue("microphone_device");
  const setMicrophoneDevice = useSetSettingValue("microphone_device");

  const { data: devices = [] } = useQuery({
    queryKey: ["settings", "microphone-devices"],
    queryFn: transcriptionCommands.listMicrophoneDevices,
    select: (result) => {
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
  });

  const options: SearchableSelectOption[] = useMemo(
    () => [
      { value: SYSTEM_DEFAULT, label: t`System default` },
      ...devices.map((name) => ({ value: name, label: name })),
    ],
    [devices, t],
  );

  return (
    <div className="flex flex-row items-center justify-between">
      <div>
        <h3 className="mb-1 text-sm font-medium">
          <Trans>Microphone</Trans>
        </h3>
        <p className="text-muted-foreground text-xs">
          <Trans>Choose which input device Anarlog records from</Trans>
        </p>
      </div>
      <SearchableSelect
        value={value ?? SYSTEM_DEFAULT}
        onChange={(val) => setMicrophoneDevice(val)}
        options={options}
        placeholder={t`System default`}
        searchPlaceholder={t`Search microphone...`}
        emptyMessage={t`No microphones found.`}
        className="w-48"
        dropdownClassName="w-72"
      />
    </div>
  );
}
